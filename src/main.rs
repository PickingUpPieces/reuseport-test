use std::{collections::HashMap, os::fd::AsRawFd, str::FromStr, thread::{self, JoinHandle}};
use nix::{fcntl::{FcntlArg, OFlag}, libc::close, sys::socket::{sockopt::ReusePort, *}, sys::select::{select, FdSet}, sys::time::TimeVal};
use nix::fcntl::fcntl;
use clap::Parser;
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};

const PORT: u16 = 45103;
const DEAULT_THREADS: usize = 4;
const MESSAGES_PER_CLIENT: usize = 100;
const DEFAULT_ROUNDS: usize = 1;
const TIMEOUT: i64 = 10;

#[derive(Debug, Serialize, Deserialize)]
struct Message {
    thread: usize,
    port: u16,
}

#[derive(Debug, Parser, Default)]
#[clap(name = "reuseport")]
pub struct Reuseport {
    /// Run as server
    #[arg(short, default_value_t = false)]
    pub server: bool,

    /// Number of threads (client or server) to create
    #[arg(short, default_value_t = DEAULT_THREADS)]
    pub threads: usize,

    /// Only client: Number of rounds to recreate the clients to send messages
    #[arg(short, default_value_t = DEFAULT_ROUNDS)]
    pub rounds: usize,
}

fn main() {
    let _ = env_logger::try_init();
    let args: Reuseport = Reuseport::parse();

    if args.server {
        info!("Starting reuseport as server");
        server(args.threads);
    } else {
        info!("Starting reuseport as client");
        client(args.threads, args.rounds);
    }
}

fn server(threads: usize) {
    let mut handles: Vec<JoinHandle<HashMap<u16, usize>>> = Vec::new(); 
    let mut counter = 0;
    for i in 0..threads {
        let thread_id = counter;
        counter += 1;
        handles.push(std::thread::spawn(move || {
            let socket = socket(AddressFamily::Inet, SockType::Datagram, SockFlag::empty(), None).expect("Creating socket failed!");
            let addr = format!("127.0.0.1:{}", PORT);
            let addr = SockaddrIn::from_str(&addr).expect("Failed to parse socketaddr");

            // Set the socket to non-blocking mode
            fcntl(socket.as_raw_fd(), FcntlArg::F_SETFL(OFlag::O_NONBLOCK)).expect("Failed to set socket to non-blocking mode");
            setsockopt(&socket, ReusePort, &true).expect("Setting SO_REUSEPORT failed");
            bind(socket.as_raw_fd(), &addr).expect("Failed to bind listener");

            let mut map: HashMap<u16, usize>= HashMap::new();
            let mut buf = [0u8; 1024];
            let mut fd_set = FdSet::new();
            fd_set.insert(&socket);
            let mut timeout = TimeVal::new(TIMEOUT, 0);

            // Wait for the first packet
            match select(socket.as_raw_fd() + 1, Some(&mut fd_set), None, None, Some(&mut timeout)) {
                Ok(n) if n > 0 => {
                    info!("THREAD {}: Received first message: {:?}", i, map);
                },
                _ => {
                    map.entry(thread_id).or_insert(0);
                    return map
                },
            }

            let mut timeout = TimeVal::new(TIMEOUT, 0);

            loop {
                // Wait for data or timeout
                match select(socket.as_raw_fd() + 1, Some(&mut fd_set), None, None, Some(&mut timeout)) {
                    Ok(n) if n > 0 => {
                        match recv(socket.as_raw_fd(), &mut buf, MsgFlags::empty()) {
                            Ok(_) => {},
                            Err(err) => {
                                error!("Failed to receive message: {}", err);
                                return map;
                            }
                        };
                        //let msg: Message = serde_json::from_str(std::str::from_utf8(&buf[..len]).unwrap()).unwrap();

                        map.entry(thread_id).and_modify(|count| *count += 1).or_insert(1);
                        debug!("THREAD {}: {:?}", i, map);
                    },
                    _ => {
                        // Error occurred
                        warn!("Error or no new messages after 1 second, finishing up...");
                        unsafe { close(socket.as_raw_fd()); }
                        return map;
                    }
                }
            }
        }));
    }

    let mut result_map: HashMap<u16, usize> = HashMap::new();
    handles.into_iter().for_each(|handle| {
        let map = handle.join().unwrap();
        for (port, count) in map {
            result_map.entry(port).and_modify(|c| *c += count).or_insert(count);
        }
    });
    println!("Results:");
    for (port, count) in result_map {
        println!("Thread {}: Received {} messages", port, count);
    }
}

fn client(threads: usize, rounds: usize) {
    let mut handles: Vec<JoinHandle<bool>> = Vec::new(); 
    for i in 0..rounds {
        info!("Round {}", i + 1);
        for i in 0..threads {
            handles.push(std::thread::spawn(move || {
                let socket = socket(AddressFamily::Inet, SockType::Datagram, SockFlag::empty(), None).expect("Creating socket failed!");
                let addr = format!("127.0.0.1:{}", PORT);
                let addr = SockaddrIn::from_str(&addr).expect("Failed to parse socketaddr");
                connect(socket.as_raw_fd(), &addr).expect("Connecting to server failed");

                let sockaddr: SockaddrIn = getsockname(socket.as_raw_fd()).expect("Failed to get socket address");
                let port = sockaddr.port();

                let msg = Message { thread: i, port };

                for _ in 0..MESSAGES_PER_CLIENT {
                    let msg_json = serde_json::to_string(&msg).unwrap();
                    if let Err(_) = send(socket.as_raw_fd(), msg_json.as_bytes(), MsgFlags::empty()) {
                        return false;
                    }
                    debug!("Sent message: {}", msg_json);
                    thread::sleep(std::time::Duration::from_millis(10));
                }
                return true;
            }));
        }
        let result = handles.into_iter().all(|handle| handle.join().unwrap());
        if !result {
            error!("Failed to send messages! Probably start the server first!");
            return
        }
        handles = Vec::new();
    }
}
