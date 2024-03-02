use enum_as_inner::EnumAsInner;
use maybe_async::maybe_async;

use std::io::{self, Read, Seek, SeekFrom};

use crate::{
    channel::{channel, ChanRecv, ChanSend},
    data::{Data, PartialItem},
    engine::{Engine, MbonParserRead},
    errors::{MbonError, MbonResult},
    marks::Mark,
};

#[cfg(feature = "sync")]
use std::thread::{spawn, JoinHandle};
#[cfg(feature = "async-tokio")]
use tokio::task::{spawn, JoinHandle};

#[derive(EnumAsInner)]
enum RequestE {
    ParseMark {
        location: SeekFrom,
    },
    ParseItem {
        location: SeekFrom,
    },
    ParseData {
        mark: Mark,
        location: SeekFrom,
    },
    ParseItemN {
        location: SeekFrom,
        count: Option<usize>,
        bytes: u64,
        parse_data: bool,
    },
    ParseDataN {
        mark: Mark,
        location: SeekFrom,
        n: usize,
    },
    Close,
}
pub struct Request {
    response: ChanSend<Response>,
    request: RequestE,
}

#[derive(EnumAsInner)]
pub enum Response {
    ParseMark(MbonResult<(Mark, u64)>),
    ParseItem(MbonResult<PartialItem>),
    ParseData(MbonResult<(Data, u64)>),
    ParseDataN(MbonResult<Vec<Data>>),
    ParseItemN(MbonResult<Vec<PartialItem>>),
    Stopped,
}

pub struct ConcurrentEngineWrapper<F> {
    engine: Engine<F>,
    recv: ChanRecv<Request>,
}

pub struct ConcurrentEngineClient {
    req: ChanSend<Request>,
}

impl Clone for ConcurrentEngineClient {
    fn clone(&self) -> Self {
        Self::new(self.req.clone())
    }
}

#[maybe_async]
impl ConcurrentEngineClient {
    pub fn new(req: ChanSend<Request>) -> Self {
        Self { req }
    }

    async fn send_request(&self, request: RequestE) -> io::Result<Response> {
        let (send, resp) = channel();
        self.req
            .send(Request {
                response: send,
                request,
            })
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::ConnectionReset, "Receiver was closed"))?;
        resp.recv()
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::ConnectionReset, "Transmitter was closed"))
    }

    fn expect<T>(value: Result<MbonResult<T>, Response>) -> MbonResult<T> {
        match value {
            Ok(res) => res,
            Err(_) => Err(MbonError::InternalError("Received Invalid Response".into())),
        }
    }

    pub async fn close_engine(&self) -> MbonResult<()> {
        let resp = self.send_request(RequestE::Close).await?;
        if !resp.is_stopped() {
            return Err(MbonError::InternalError("Received Invalid Response".into()));
        }
        Ok(())
    }
}

#[maybe_async]
impl MbonParserRead for ConcurrentEngineClient {
    async fn parse_mark(&mut self, location: SeekFrom) -> MbonResult<(Mark, u64)> {
        let response = self.send_request(RequestE::ParseMark { location }).await?;
        Self::expect(response.into_parse_mark())
    }

    async fn parse_data(&mut self, mark: &Mark, location: SeekFrom) -> MbonResult<(Data, u64)> {
        let response = self
            .send_request(RequestE::ParseData {
                mark: mark.to_owned(),
                location,
            })
            .await?;
        Self::expect(response.into_parse_data())
    }

    async fn parse_item(&mut self, location: SeekFrom) -> MbonResult<PartialItem> {
        let response = self.send_request(RequestE::ParseItem { location }).await?;
        Self::expect(response.into_parse_item())
    }

    async fn parse_item_n(
        &mut self,
        location: SeekFrom,
        count: Option<usize>,
        bytes: u64,
        parse_data: bool,
    ) -> MbonResult<Vec<PartialItem>> {
        let response = self
            .send_request(RequestE::ParseItemN {
                location,
                count,
                bytes,
                parse_data,
            })
            .await?;
        Self::expect(response.into_parse_item_n())
    }

    async fn parse_data_n(
        &mut self,
        mark: &Mark,
        location: SeekFrom,
        n: usize,
    ) -> MbonResult<Vec<Data>> {
        let response = self
            .send_request(RequestE::ParseDataN {
                mark: mark.to_owned(),
                location,
                n,
            })
            .await?;
        Self::expect(response.into_parse_data_n())
    }
}

/// Wraps an [crate::engine::Engine] allowing for multiple concurrent requests
/// to the engine
#[maybe_async]
impl<F> ConcurrentEngineWrapper<F>
where
    F: Read + Seek + Send + 'static,
{
    pub fn new(engine: Engine<F>) -> (Self, ConcurrentEngineClient) {
        let (send, recv) = channel();
        (Self { engine, recv }, ConcurrentEngineClient::new(send))
    }

    pub fn spawn(self) -> JoinHandle<io::Result<()>> {
        #[cfg(feature = "async")]
        let future = self.program_loop();
        #[cfg(feature = "sync")]
        let future = || self.program_loop();
        return spawn(future);
    }

    async fn program_loop(mut self) -> io::Result<()> {
        loop {
            let action = self.recv.recv().await.map_err(|_| {
                io::Error::new(io::ErrorKind::ConnectionReset, "Transmitter was closed")
            })?;
            if self.on_action(action).await {
                return Ok(());
            }
        }
    }

    async fn on_action(&mut self, action: Request) -> bool {
        let response = match action.request {
            RequestE::ParseMark { location } => {
                Response::ParseMark(self.engine.parse_mark(location).await)
            }
            RequestE::ParseItem { location } => {
                Response::ParseItem(self.engine.parse_item(location).await)
            }
            RequestE::ParseData { mark, location } => {
                Response::ParseData(self.engine.parse_data(&mark, location).await)
            }
            RequestE::ParseDataN { mark, location, n } => {
                Response::ParseDataN(self.engine.parse_data_n(&mark, location, n).await)
            }
            RequestE::ParseItemN {
                location,
                count,
                bytes,
                parse_data,
            } => Response::ParseItemN(
                self.engine
                    .parse_item_n(location, count, bytes, parse_data)
                    .await,
            ),
            RequestE::Close => {
                action.response.send(Response::Stopped).await.ok();
                return true;
            }
        };

        action.response.send(response).await.ok();
        false
    }
}
