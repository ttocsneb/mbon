use enum_as_inner::EnumAsInner;
use maybe_async::maybe_async;

use crate::{
    channel::{channel, ChanRecv, ChanSend},
    data::{Data, PartialItem},
    engine::{Engine, MbonParserRead},
    errors::{MbonError, MbonResult},
    marks::Mark,
    stream::{Reader, Seeker},
};

use std::io;

#[cfg(feature = "sync")]
use std::thread::{spawn, JoinHandle};
#[cfg(feature = "async-tokio")]
use tokio::task::{spawn, JoinHandle};

#[derive(EnumAsInner)]
enum RequestE {
    ParseMark { location: u64 },
    ParseItem { location: u64 },
    ParseData { mark: Mark, location: u64 },
    ParseItemFull { location: u64 },
    ParseDataFull { mark: Mark, location: u64 },
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
    ParseData(MbonResult<Data>),
    ParseDataFull(MbonResult<Data>),
    ParseItemFull(MbonResult<PartialItem>),
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
    async fn parse_mark(&mut self, location: u64) -> MbonResult<(Mark, u64)> {
        let response = self.send_request(RequestE::ParseMark { location }).await?;
        Self::expect(response.into_parse_mark())
    }

    async fn parse_data(&mut self, mark: &Mark, location: u64) -> MbonResult<Data> {
        let response = self
            .send_request(RequestE::ParseData {
                mark: mark.to_owned(),
                location,
            })
            .await?;
        Self::expect(response.into_parse_data())
    }

    async fn parse_item(&mut self, location: u64) -> MbonResult<PartialItem> {
        let response = self.send_request(RequestE::ParseItem { location }).await?;
        Self::expect(response.into_parse_item())
    }

    async fn parse_data_full(&mut self, mark: &Mark, location: u64) -> MbonResult<Data> {
        let response = self
            .send_request(RequestE::ParseDataFull {
                mark: mark.to_owned(),
                location,
            })
            .await?;
        Self::expect(response.into_parse_data_full())
    }

    async fn parse_item_full(&mut self, location: u64) -> MbonResult<PartialItem> {
        let response = self
            .send_request(RequestE::ParseItemFull { location })
            .await?;
        Self::expect(response.into_parse_item_full())
    }
}

/// Wraps an [crate::engine::Engine] allowing for multiple concurrent requests
/// to the engine
#[maybe_async]
impl<F> ConcurrentEngineWrapper<F>
where
    F: Reader + Seeker + Send + 'static,
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
            RequestE::ParseItemFull { location } => {
                Response::ParseItemFull(self.engine.parse_item_full(location).await)
            }
            RequestE::ParseDataFull { mark, location } => {
                Response::ParseDataFull(self.engine.parse_data_full(&mark, location).await)
            }
            RequestE::Close => {
                action.response.send(Response::Stopped).await.ok();
                return true;
            }
        };

        action.response.send(response).await.ok();
        false
    }
}
