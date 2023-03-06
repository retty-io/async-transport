#[cfg(feature = "metal-io")]
#[cfg(test)]
mod tests {
    use async_transport::{Capabilities, EcnCodepoint, RecvMeta, Transmit, UdpSocket, BATCH_SIZE};
    use retty_io::{Events, Poll, PollOpt, Ready, Token};
    use std::io::IoSliceMut;
    use std::net::Ipv4Addr;
    use std::thread;
    use std::time::{Duration, Instant};

    #[test]
    fn test_ecn() -> std::io::Result<()> {
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

        let handle = thread::spawn(move || {
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
            println!("sending {:?}", meta[0].ecn);
            let _ = tx.send(meta[0].ecn);
            println!("sent {:?}", meta[0].ecn);

            Ok::<(), std::io::Error>(())
        });

        let start = Instant::now();

        thread::sleep(Duration::from_millis(500));

        const SOCKET_WT: Token = Token(0);
        let poll = Poll::new()?;
        let mut events = Events::with_capacity(2);
        poll.register(&socket1, SOCKET_WT, Ready::writable(), PollOpt::edge())?;
        poll.poll(&mut events, None)?;
        for event in events.iter() {
            match event.token() {
                SOCKET_WT => {
                    let n = socket1.send(&capabilities, &transmits)?;
                    println!("sent {} packets in {}ms", n, start.elapsed().as_millis());
                }
                _ => unreachable!(),
            }
        }

        let _ = handle.join();

        println!("receiving ecn");
        let ecn = rx.recv().unwrap();
        println!("receiving {:?}", ecn);

        #[cfg(not(windows))]
        {
            assert!(ecn.is_some());
            assert_eq!(EcnCodepoint::Ce, ecn.unwrap());
        }
        #[cfg(windows)]
        assert!(ecn.is_none());

        Ok(())
    }

    #[test]
    pub fn test_udp() {
        let socket1 = retty_io::net::UdpSocket::bind(&"127.0.0.1:0".parse().unwrap()).unwrap();
        let socket2 = retty_io::net::UdpSocket::bind(&"127.0.0.1:0".parse().unwrap()).unwrap();
        let addr2 = socket2.local_addr().unwrap();

        let contents = (12324343 as u64).to_be_bytes().to_vec();

        let (tx, rx) = std::sync::mpsc::channel();

        let handle = std::thread::spawn(move || {
            let mut buffers = vec![0, 0, 0, 0, 0, 0, 0, 0];

            const SOCKET_RD: Token = Token(0);
            let poll = Poll::new()?;
            let mut events = Events::with_capacity(2);
            poll.register(&socket2, SOCKET_RD, Ready::readable(), PollOpt::edge())
                .unwrap();
            poll.poll(&mut events, None)?;
            for event in events.iter() {
                match event.token() {
                    SOCKET_RD => {
                        let n = socket2.recv(&mut buffers).unwrap();
                        println!("received {} {:?}", n, buffers);
                    }
                    _ => unreachable!(),
                }
            }
            println!("sending {:?}", buffers);
            let _ = tx.send(Some(buffers));
            println!("sent");

            Ok::<(), std::io::Error>(())
        });

        let start = Instant::now();

        std::thread::sleep(Duration::from_millis(500));

        const SOCKET_WT: Token = Token(0);
        let poll = Poll::new().unwrap();
        let mut events = Events::with_capacity(2);
        poll.register(&socket1, SOCKET_WT, Ready::writable(), PollOpt::edge())
            .unwrap();
        poll.poll(&mut events, None).unwrap();
        for event in events.iter() {
            match event.token() {
                SOCKET_WT => {
                    let n = socket1.send_to(&contents, &addr2).unwrap();
                    println!("sent {} packets in {}ms", n, start.elapsed().as_millis());
                }
                _ => unreachable!(),
            }
        }

        let _ = handle.join();

        println!("receiving ecn");
        let ecn = rx.recv().unwrap();
        println!("receiving {:?}", ecn);
        assert!(ecn.is_some());
    }
}
