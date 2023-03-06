use crate::{Capabilities, RecvMeta, Transmit, UdpSocketState};
use retty_io::{lazycell::AtomicLazyCell, Evented, Poll, PollOpt, Ready, Token};
use std::{
    io,
    net::{SocketAddr, ToSocketAddrs},
};

#[derive(Debug)]
pub struct UdpSocket {
    io: retty_io::net::UdpSocket,
    inner: UdpSocketState,
    peer: AtomicLazyCell<SocketAddr>,
}

impl Evented for UdpSocket {
    fn register(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> io::Result<()> {
        self.io.register(poll, token, interest, opts)
    }

    fn reregister(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> io::Result<()> {
        self.io.reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        self.io.deregister(poll)
    }
}

impl UdpSocket {
    pub fn bind<A: ToSocketAddrs>(addrs: A) -> io::Result<Self> {
        let mut last_err = None;
        let addrs = addrs.to_socket_addrs()?;

        for addr in addrs {
            match retty_io::net::UdpSocket::bind(&addr) {
                Ok(socket) => {
                    UdpSocketState::configure((&socket).into())?;
                    return Ok(Self {
                        io: socket,
                        inner: UdpSocketState::new(),
                        peer: AtomicLazyCell::new(),
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

    pub fn connect<A: ToSocketAddrs>(&self, addrs: A) -> io::Result<()> {
        let mut last_err = None;
        let addrs = addrs.to_socket_addrs()?;

        for addr in addrs {
            match self.io.connect(addr) {
                Ok(()) => {
                    self.peer.fill(addr).map_err(|_| {
                        io::Error::new(io::ErrorKind::AddrInUse, "peer address existed")
                    })?;
                    return Ok(());
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

    pub fn send_to<A: ToSocketAddrs>(&self, buf: &[u8], addrs: A) -> io::Result<usize> {
        let addr = match addrs.to_socket_addrs()?.next() {
            Some(addr) => addr,
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "no addresses to send data to",
                ));
            }
        };

        self.io.send_to(buf, &addr)
    }

    pub fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.io.recv_from(buf)
    }

    pub fn send(
        &self,
        #[allow(unused_variables)] capabilities: &Capabilities,
        transmits: &[Transmit],
    ) -> io::Result<usize> {
        #[cfg(windows)]
        {
            let mut sent = 0;
            for transmit in transmits {
                match self.io.send_to(&transmit.contents, &transmit.destination) {
                    Ok(_) => {
                        sent += 1;
                    }
                    // We need to report that some packets were sent in this case, so we rely on
                    // errors being either harmlessly transient (in the case of WouldBlock) or
                    // recurring on the next call.
                    Err(_) if sent != 0 => return Ok(sent),
                    Err(e) => {
                        if e.kind() == io::ErrorKind::WouldBlock {
                            return Err(e);
                        }

                        // Other errors are ignored, since they will ususally be handled
                        // by higher level retransmits and timeouts.
                        //log_sendmsg_error(&self.epoch, &self.last_send_error, e, transmit);
                        sent += 1;
                    }
                }
            }
            Ok(sent)
        }
        #[cfg(not(windows))]
        self.inner.send((&self.io).into(), capabilities, transmits)
    }

    pub fn recv(
        &self,
        bufs: &mut [io::IoSliceMut<'_>],
        meta: &mut [RecvMeta],
    ) -> io::Result<usize> {
        #[cfg(windows)]
        {
            let (len, addr) = self.io.recv_from(&mut bufs[0])?;
            meta[0] = RecvMeta {
                len,
                stride: len,
                addr,
                ecn: None,
                dst_ip: None,
            };
            Ok(1)
        }
        #[cfg(not(windows))]
        self.inner.recv((&self.io).into(), bufs, meta)
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.io.local_addr()
    }

    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        if let Some(peer) = self.peer.borrow() {
            Ok(*peer)
        } else {
            Err(io::Error::new(io::ErrorKind::AddrNotAvailable, ""))
        }
    }
}
