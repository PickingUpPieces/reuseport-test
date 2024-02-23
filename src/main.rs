use std::{os::fd::AsRawFd, str::FromStr, thread::JoinHandle};
use nix::sys::socket::{sockopt::ReusePort, *};
use clap::Parser;
use log::info;

const PORT: u16 = 45103;
const SERVER_THREADS: usize = 5;
const CLIENT_THREADS: usize = 2;

#[derive(Debug, Parser, Default)]
#[clap(name = "reuseport")]
pub struct Reuseport {
    #[arg(short, default_value_t = false)]
    pub server: bool,
}

fn main() {
    let _ = env_logger::try_init();
    let args: Reuseport = Reuseport::parse();

    if args.server {
        info!("Starting reuseport as server");
        server();
    } else {
        info!("Starting reuseport as client");
        client();
    }
}

fn server() {
    let mut handles: Vec<JoinHandle<()>> = Vec::new(); 
    for i in 0..SERVER_THREADS {
        handles.push(std::thread::spawn(move || {
            let socket = socket(AddressFamily::Inet, SockType::Datagram, SockFlag::empty(), None).expect("Creating socket failed!");
            let addr = format!("127.0.0.1:{}", PORT);
            let addr = SockaddrIn::from_str(&addr).expect("Failed to parse socketaddr");
            
            setsockopt(&socket, ReusePort, &true).expect("Setting SO_REUSEPORT failed");
            bind(socket.as_raw_fd(), &addr).expect("Failed to bind listener");

            // Receive in a loop
            loop {
                let mut buf = [0u8; 1024];
                let len = recv(socket.as_raw_fd(), &mut buf, MsgFlags::empty()).expect("Failed to receive message");
                info!("Received on THREAD {}, port {}: {:?}", i, PORT, std::str::from_utf8(&buf[..len]).unwrap());
            }
        }));
    }

    handles.into_iter().for_each(|handle| handle.join().unwrap());
}

fn client() {
    let mut handles: Vec<JoinHandle<()>> = Vec::new(); 
    for i in 0..CLIENT_THREADS {
        handles.push(std::thread::spawn(move || {
            let socket = socket(AddressFamily::Inet, SockType::Datagram, SockFlag::empty(), None).expect("Creating socket failed!");
            let addr = format!("127.0.0.1:{}", PORT);
            let addr = SockaddrIn::from_str(&addr).expect("Failed to parse socketaddr");
            connect(socket.as_raw_fd(), &addr).expect("Connecting to server failed");

            let sockaddr: SockaddrIn = getsockname(socket.as_raw_fd()).expect("Failed to get socket address");
            let port = sockaddr.port();

            for _ in 0..10 {
                let msg = format!("THREAD {}: port {}", i, port);
                send(socket.as_raw_fd(), msg.as_bytes(), MsgFlags::empty()).expect("Failed to send message");
                info!("Sent message: {}", msg);
            }
        }));
    }

    handles.into_iter().for_each(|handle| handle.join().unwrap());
}
