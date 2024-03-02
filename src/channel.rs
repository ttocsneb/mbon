use maybe_async::maybe_async;

#[cfg(feature = "async")]
use async_channel::{unbounded as new_channel, Receiver, RecvError, SendError, Sender};
#[cfg(feature = "sync")]
use std::sync::mpsc::{channel as new_channel, Receiver, RecvError, SendError, Sender};

pub struct ChanSend<T>(pub Sender<T>);
pub struct ChanRecv<T>(pub Receiver<T>);

impl<T> Clone for ChanSend<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

#[maybe_async]
pub fn channel<T>() -> (ChanSend<T>, ChanRecv<T>) {
    let (s, r) = new_channel();
    (ChanSend(s), ChanRecv(r))
}

#[maybe_async]
impl<T> ChanSend<T> {
    #[inline]
    pub async fn send(&self, data: T) -> Result<(), SendError<T>> {
        self.0.send(data).await
    }

    #[cfg(feature = "async")]
    #[inline]
    pub fn send_blocking(&self, data: T) -> Result<(), SendError<T>> {
        self.0.send_blocking(data)
    }

    #[cfg(feature = "sync")]
    #[inline]
    pub fn send_blocking(&self, data: T) -> Result<(), SendError<T>> {
        self.0.send(data)
    }
}
#[maybe_async]
impl<T> ChanRecv<T> {
    #[inline]
    pub async fn recv(&self) -> Result<T, RecvError> {
        self.0.recv().await
    }

    #[cfg(feature = "async")]
    #[inline]
    pub fn recv_blocking(&self) -> Result<T, RecvError> {
        self.0.recv_blocking()
    }

    #[cfg(feature = "sync")]
    #[inline]
    pub fn recv_blocking(&self) -> Result<T, RecvError> {
        self.0.recv()
    }
}
