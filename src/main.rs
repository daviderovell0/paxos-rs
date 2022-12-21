
use std::env;
use std::io::{Error, BufReader, BufRead};
use std::fs::File;
use std::collections::*;
use std::net::{UdpSocket, Ipv4Addr, SocketAddrV4};
use socket2::*;

// CONSTANTS
const CONFIG_PATH: &str = "paxos.conf"; // wrt target/<dir>

// AUX FUNCTIONS
fn parse_cfg() -> Result<HashMap<String, SocketAddrV4>, Error> {

    let mut cfg = HashMap::new();

    // Open the path in read-only mode, returns `io::Result<File>`
    let file = File::open(CONFIG_PATH)?;
    let lines = BufReader::new(file).lines(); //read line by line
    
    for line in lines {
        if let Ok(ip) = line {
            let los: Vec<&str> = ip.split_whitespace().collect();
            cfg.insert(los[0].to_string(), 
            SocketAddrV4::new(los[1].parse().unwrap(), los[2].parse().unwrap()));
        } //not checking errors
    }
    
    Ok(cfg)
}

fn mcast_receiver(address: SocketAddrV4) -> Socket {
    // UNSPECIFIED address = make to OS choose the address
    // equivalent to INADDR_ANY
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, None)
    .expect("failed to create socket");

    //let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).unwrap();

    socket
    .join_multicast_v4(address.ip(), &Ipv4Addr::UNSPECIFIED)
    .expect("failed to join multicast group");

    socket.set_reuse_address(true).unwrap();

    socket.bind(&SockAddr::from(address)).expect("failed to join multicast group");
    socket
}

fn mcast_sender() -> UdpSocket {
    // UNSPECIFIED address = make to OS choose the address
    // equivalent to INADDR_ANY
    UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).unwrap()
}

// PAXOS ROLES
fn acceptor(mut cfg: HashMap<String, SocketAddrV4>, id: u16) {
    println!("> acceptor {}", id);
    let s = mcast_sender();
    let r = mcast_receiver(cfg.remove("acceptors").unwrap());
}

fn proposer(cfg: HashMap<String, SocketAddrV4>, id: u16) {
    println!("Hello from proposer");
}

fn learner(cfg: HashMap<String, SocketAddrV4>, id: u16) {
    println!("Hello from learner");
}

fn client(cfg: HashMap<String, SocketAddrV4>, id: u16) {
    println!("Hello from client");
    println!("{:?}", cfg);
}


fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 3 {
        println!("Usage: paxos-rs <role> <id>");
        return;
    }
    let role = args[1].as_str();
    let id = args[2].parse().unwrap();

    let cfg = match parse_cfg() {
        Ok(h) => h,
        Err(e) => panic!("Failed to parse the configuration file. Err: {}", e),
    };
    
    //println!("{:?}", cfg);

    match role {
        "acceptor" => acceptor(cfg, id),
        "learner" => learner(cfg, id),
        "client" => client(cfg, id),
        "proposer" => proposer(cfg, id),
        _ => println!("Invalid role: {}", role)

    }
}
