
use std::env;
use std::io::{Error, BufReader, BufRead, stdout, Write};
use std::fs::File;
use std::collections::*;
use std::net::{UdpSocket, Ipv4Addr, SocketAddrV4};
use socket2::*;
use std::mem::MaybeUninit;

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

fn mcast_receiver(address: &SocketAddrV4) -> Socket {
    // UNSPECIFIED address = make to OS choose the address
    // equivalent to INADDR_ANY
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))
    .expect("failed to create socket");

    println!("{:?}",address.ip());

    socket
    .join_multicast_v4(address.ip(), &Ipv4Addr::UNSPECIFIED)
    .expect("failed to join multicast group");

    socket.set_reuse_address(true).expect("failed to set reuse address");
    socket.bind(&SockAddr::from(address.to_owned())).expect("failed to bind");
    socket
}

fn mcast_sender() -> UdpSocket {
    // UNSPECIFIED address = make to OS choose the address
    // equivalent to INADDR_ANY
    UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).unwrap()
}

fn paxos_encode(lon: &Vec<i32>) -> Vec<u8> {
    // get an list of numbers
    // convert each element to bytes (big endian for network)
    // ungroup the bytes arrays (-> flatten) 
    // put everything in a vector 
    lon.iter()
    .map(|x| x.to_be_bytes())
    .flatten()
    .collect()
}

fn paxos_decode(byte_array: &[MaybeUninit<u8>] , size: usize) -> Vec<i32> {

    // use step by
    for i in 0..size {
        let x = unsafe {byte_array[i].assume_init()};
        println!("{}",x);
    };

}

// PAXOS ROLES
fn acceptor(cfg: HashMap<String, SocketAddrV4>, id: u16) {
    println!("> acceptor {}", id);
    let s = mcast_sender();
    let r = mcast_receiver(cfg.get("acceptors").unwrap());

    loop {
        let mut recvbuf = [MaybeUninit::new(0); 64];
        let (bytes_n, src_addr) = r.recv_from(&mut recvbuf)
                                        .expect("Didn't receive data");
        println!("bytes recv: {} from {}", bytes_n, 
        src_addr.as_socket_ipv4().unwrap().ip());

        for i in 0..bytes_n {
            let x = unsafe {recvbuf[i].assume_init()};
            println!("{}",x);
        };

        //println!("bufrecv: {:?}", recvbuf);
        
        stdout().flush().unwrap()
    }
}

fn proposer(cfg: HashMap<String, SocketAddrV4>, id: u16) {
    println!("> proposer {}", id);
    let s = mcast_sender();
    let r = mcast_receiver(cfg.get("proposers").unwrap());

    loop {
        let num: i32 = 100000;
        let buf = num.to_be_bytes();
        match s.send_to(&buf, cfg.get("acceptors").unwrap()) {
            Ok(bytes_sent) => println!("sent {} bytes", bytes_sent),
            Err(e) => panic!("couldn't send from proposer, err: {}", e)
        }
        return;
    }
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
