#[cfg(feature = "metal-io")]
#[cfg(test)]
mod tests {
    use anyhow::Result;
    use async_transport::{Capabilities, EcnCodepoint, RecvMeta, Transmit, UdpSocket, BATCH_SIZE};
    use mio::{Events, Poll, PollOpt, Ready, Token};
    use std::io::IoSliceMut;
    use std::net::Ipv4Addr;
    use std::thread;
    use std::time::Instant;

    #[test]
    fn test_ecn() -> Result<()> {
        env_logger::init();
        let capabilities = Capabilities::new();
        let socket1 = UdpSocket::bind("127.0.0.1:0")?;
        let socket2 = UdpSocket::bind("127.0.0.1:0")?;
        let addr2 = socket2.local_addr()?;

        let mut transmits = Vec::with_capacity(1);
        for i in 0..1 {
            let contents = (i as u64).to_be_bytes().to_vec();
            transmits.push(Transmit {
                destination: addr2,
                ecn: Some(EcnCodepoint::Ce),
                segment_size: None,
                contents,
                src_ip: Some(Ipv4Addr::LOCALHOST.into()),
            });
        }

        let (tx, rx) = std::sync::mpsc::channel();

        thread::spawn(move || {
            let mut storage = [[0u8; 1200]; BATCH_SIZE];
            let mut buffers = Vec::with_capacity(BATCH_SIZE);
            let mut rest = &mut storage[..];
            for _ in 0..BATCH_SIZE {
                let (b, r) = rest.split_at_mut(1);
                rest = r;
                buffers.push(IoSliceMut::new(&mut b[0]));
            }

            let mut meta = [RecvMeta::default(); BATCH_SIZE];

            const SOCKET_RD: Token = Token(0);
            let poll = Poll::new()?;
            let mut events = Events::with_capacity(2);
            poll.register(&socket2, SOCKET_RD, Ready::readable(), PollOpt::edge())?;
            poll.poll(&mut events, None)?;
            for event in events.iter() {
                match event.token() {
                    SOCKET_RD => {
                        let n = socket2.recv(&mut buffers, &mut meta)?;
                        for i in 0..n {
                            println!(
                                "received {} {:?} {:?}",
                                i,
                                &buffers[i][..meta[i].len],
                                &meta[i]
                            );
                        }
                    }
                    _ => unreachable!(),
                }
            }

            let _ = tx.send(meta[0].ecn);

            Ok::<(), std::io::Error>(())
        });

        let start = Instant::now();

        log::debug!("before send");
        socket1.send(&capabilities, &transmits)?;
        log::debug!("after send");

        println!("sent {} packets in {}ms", 1, start.elapsed().as_millis());

        let ecn = rx.recv().unwrap();

        #[cfg(not(windows))]
        {
            assert!(ecn.is_some());
            assert_eq!(EcnCodepoint::Ce, ecn.unwrap());
        }

        Ok(())
    }
}
