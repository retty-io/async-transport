use crate::runtime::AsyncUdpSocket;
use crate::{RecvMeta, Transmit, UdpSocketState, UdpState};
use std::{
    io,
    task::{Context, Poll},
};
use tokio::io::Interest;

#[derive(Debug)]
struct UdpSocket {
    io: tokio::net::UdpSocket,
    inner: UdpSocketState,
}

impl AsyncUdpSocket for UdpSocket {
    fn poll_send(
        &mut self,
        state: &UdpState,
        cx: &mut Context<'_>,
        transmits: &[Transmit],
    ) -> Poll<io::Result<usize>> {
        let inner = &mut self.inner;
        let io = &self.io;
        loop {
            ready!(io.poll_send_ready(cx))?;
            if let Ok(res) = io.try_io(Interest::WRITABLE, || {
                inner.send(io.into(), state, transmits)
            }) {
                return Poll::Ready(Ok(res));
            }
        }
    }

    fn poll_recv(
        &self,
        cx: &mut Context<'_>,
        bufs: &mut [std::io::IoSliceMut<'_>],
        meta: &mut [RecvMeta],
    ) -> Poll<io::Result<usize>> {
        loop {
            ready!(self.io.poll_recv_ready(cx))?;
            if let Ok(res) = self.io.try_io(Interest::READABLE, || {
                self.inner.recv((&self.io).into(), bufs, meta)
            }) {
                return Poll::Ready(Ok(res));
            }
        }
    }

    fn local_addr(&self) -> io::Result<std::net::SocketAddr> {
        self.io.local_addr()
    }
}
