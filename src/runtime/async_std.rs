use crate::runtime::AsyncUdpSocket;
use crate::{RecvMeta, Transmit, UdpSocketState, UdpState};
use async_io::Async;
use async_std::net::ToSocketAddrs;
use std::{
    future::poll_fn,
    io,
    net::SocketAddr,
    task::{Context, Poll},
};

#[derive(Debug)]
pub struct UdpSocket {
    io: Async<std::net::UdpSocket>,
    inner: UdpSocketState,
}

impl AsyncUdpSocket for UdpSocket {
    fn poll_send(
        &self,
        cx: &mut Context<'_>,
        state: &UdpState,
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

impl UdpSocket {
    pub async fn bind<A: ToSocketAddrs>(addrs: A) -> io::Result<Self> {
        let mut last_err = None;
        let addrs = addrs.to_socket_addrs().await?;

        for addr in addrs {
            match Async::<std::net::UdpSocket>::bind(addr) {
                Ok(socket) => {
                    return Ok(Self {
                        io: socket,
                        inner: UdpSocketState::new(),
                    });
                }
                Err(err) => last_err = Some(err),
            }
        }

        Err(last_err.unwrap_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "could not resolve to any addresses",
            )
        }))
    }

    pub async fn connect<A: ToSocketAddrs>(&self, addrs: A) -> io::Result<()> {
        let mut last_err = None;
        let addrs = addrs.to_socket_addrs().await?;

        for addr in addrs {
            // TODO(stjepang): connect on the blocking pool
            match self.io.get_ref().connect(addr) {
                Ok(()) => return Ok(()),
                Err(err) => last_err = Some(err),
            }
        }

        Err(last_err.unwrap_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "could not resolve to any addresses",
            )
        }))
    }

    pub async fn send_to<A: ToSocketAddrs>(&self, buf: &[u8], addrs: A) -> io::Result<usize> {
        let addr = match addrs.to_socket_addrs().await?.next() {
            Some(addr) => addr,
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "no addresses to send data to",
                ));
            }
        };

        self.io.send_to(buf, addr).await
    }

    pub async fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.io.recv_from(buf).await
    }

    pub async fn send(&self, state: &UdpState, transmits: &[Transmit]) -> io::Result<usize> {
        poll_fn(|cx| self.poll_send(cx, state, transmits)).await
    }

    pub async fn recv(
        &self,
        bufs: &mut [io::IoSliceMut<'_>],
        meta: &mut [RecvMeta],
    ) -> io::Result<usize> {
        poll_fn(|cx| self.poll_recv(cx, bufs, meta)).await
    }
}
