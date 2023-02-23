use crate::runtime::AsyncUdpSocket;
use crate::{RecvMeta, Transmit, UdpSocketState, UdpState};
use async_io::Async;
use std::{
    io,
    task::{Context, Poll},
};

#[derive(Debug)]
struct UdpSocket {
    io: Async<std::net::UdpSocket>,
    inner: UdpSocketState,
}

impl AsyncUdpSocket for UdpSocket {
    fn poll_send(
        &mut self,
        state: &UdpState,
        cx: &mut Context<'_>,
        transmits: &[Transmit],
    ) -> Poll<io::Result<usize>> {
        loop {
            ready!(self.io.poll_writable(cx))?;
            if let Ok(res) = self.inner.send((&self.io).into(), state, transmits) {
                return Poll::Ready(Ok(res));
            }
        }
    }

    fn poll_recv(
        &self,
        cx: &mut Context<'_>,
        bufs: &mut [io::IoSliceMut<'_>],
        meta: &mut [RecvMeta],
    ) -> Poll<io::Result<usize>> {
        loop {
            ready!(self.io.poll_readable(cx))?;
            if let Ok(res) = self.inner.recv((&self.io).into(), bufs, meta) {
                return Poll::Ready(Ok(res));
            }
        }
    }

    fn local_addr(&self) -> io::Result<std::net::SocketAddr> {
        self.io.as_ref().local_addr()
    }
}
