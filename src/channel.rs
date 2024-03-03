//! Redefinitions of channel
//!
//! When the feature `async` is enabled, then [async_channel] will be used.
//!
//! When the feature `sync` is enabled, then [std::sync::mpsc] will be used.

use maybe_async::maybe_async;

#[cfg(feature = "async")]
use async_channel::{unbounded as new_channel, Receiver, RecvError, SendError, Sender};
#[cfg(feature = "sync")]
use std::sync::mpsc::{channel as new_channel, Receiver, RecvError, SendError, Sender};

/// The sending half of a channel
pub struct ChanSend<T>(pub Sender<T>);
/// The receiving half of a channel
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
    /// Send a message to the receiver
    #[inline]
    pub async fn send(&self, data: T) -> Result<(), SendError<T>> {
        self.0.send(data).await
    }

    /// Send a message to the receiver
    ///
    /// This is the same as [Self::send] when feature `sync` is set
    #[cfg(feature = "async")]
    #[inline]
    pub fn send_blocking(&self, data: T) -> Result<(), SendError<T>> {
        self.0.send_blocking(data)
    }

    /// Send a message to the receiver
    ///
    /// This is the same as [Self::send] when feature `sync` is set
    #[cfg(feature = "sync")]
    #[inline]
    pub fn send_blocking(&self, data: T) -> Result<(), SendError<T>> {
        self.0.send(data)
    }
}
#[maybe_async]
impl<T> ChanRecv<T> {
    /// Receive a message from a sender.
    ///
    /// This will wait until a message is ready
    #[inline]
    pub async fn recv(&self) -> Result<T, RecvError> {
        self.0.recv().await
    }

    /// Receive a message from a sender.
    ///
    /// This will wait until a message is ready
    ///
    /// This is the same as [Self::recv] when feature `sync` is set
    #[cfg(feature = "async")]
    #[inline]
    pub fn recv_blocking(&self) -> Result<T, RecvError> {
        self.0.recv_blocking()
    }

    /// Receive a message from a sender.
    ///
    /// This will wait until a message is ready
    ///
    /// This is the same as [Self::recv] when feature `sync` is set
    #[cfg(feature = "sync")]
    #[inline]
    pub fn recv_blocking(&self) -> Result<T, RecvError> {
        self.0.recv()
    }
}
