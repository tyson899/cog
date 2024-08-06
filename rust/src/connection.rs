use crate::runner::{PredictionRequest, PredictionResponse};

use futures::prelude::*;
use std::fmt::Debug;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::sync::mpsc;

type StdError = Box<dyn std::error::Error + Send + Sync + 'static>;

pub struct ChannelTransport<Request, Response> {
    rcv: mpsc::UnboundedReceiver<Request>,
    snd: mpsc::UnboundedSender<Response>,
}

impl<R, S> ChannelTransport<R, S> {
    pub fn new(rcv: mpsc::UnboundedReceiver<R>, snd: mpsc::UnboundedSender<S>) -> Self {
        Self { rcv, snd }
    }
}

impl<R, S> Sink<S> for ChannelTransport<R, S> {
    type Error = StdError;

    fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self>, item: S) -> Result<(), Self::Error> {
        self.get_mut()
            .snd
            .send(item)
            .map_err(|e| e.to_string().into())
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
}

impl<R, S> Stream for ChannelTransport<R, S> {
    type Item = Result<R, StdError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        self.rcv.poll_recv(cx).map(|s| s.map(Ok))
    }
}
