
use std::env;
use std::io::{Error, BufReader, BufRead, stdout, stdin, Write};
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

fn paxos_encode(lon: &[i32]) -> Vec<u8> {
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

    let mut lon = Vec::new();
    let mut byte_word = [0; 4];
    // use step by
    for i in 0..size {
        let x = unsafe {byte_array[i].assume_init()};
        byte_word[i%4] = x;

        if i%4 == 3 { // every 4 bytes
            lon.push(i32::from_be_bytes(byte_word));
        }
    };

    lon

}

// PAXOS ROLES
fn acceptor(cfg: HashMap<String, SocketAddrV4>, id: u16) {
    println!("> acceptor {}", id);
    let s = mcast_sender();
    let r = mcast_receiver(cfg.get("acceptors")
    .expect("no entry for key 'acceptors' in config file"));

    loop {
        let mut recvbuf = [MaybeUninit::new(0); 64];
        let (bytes_n, _src_addr) = r.recv_from(&mut recvbuf)
                                    .expect("Didn't receive data");

        let msg = paxos_decode(&recvbuf, bytes_n);

        println!("bufrecv: {:?}", msg);
        
        stdout().flush().unwrap()
    }
}

fn proposer(cfg: HashMap<String, SocketAddrV4>, id: u16) {
    println!("> proposer {}", id);
    let s = mcast_sender();
    let r = mcast_receiver(cfg.get("proposers")
    .expect("no entry for key 'proposers' in config file"));

    loop {

        let mut recvbuf = [MaybeUninit::new(0); 64];
        let (bytes_n, _src_addr) = r.recv_from(&mut recvbuf)
                                    .expect("Didn't receive data");

        let msg = paxos_decode(&recvbuf, bytes_n);

        println!("bufrecv: {:?}", msg);
        
        


        let msg = paxos_encode(&[1,2,3]);
        match s.send_to(&msg, cfg.get("acceptors").unwrap()) {
            Ok(bytes_sent) => println!("sent {} bytes", bytes_sent),
            Err(e) => panic!("couldn't send from proposer, err: {}", e)
        }
        return;

        stdout().flush().unwrap();
    }
}

fn learner(cfg: HashMap<String, SocketAddrV4>, id: u16) {
    println!("> learner {}", id);
    let s = mcast_sender();
    let r = mcast_receiver(cfg.get("learners")
    .expect("no entry for key 'learners' in config file"));

    loop {

        let mut recvbuf = [MaybeUninit::new(0); 64];
        let (bytes_n, _src_addr) = r.recv_from(&mut recvbuf)
                                    .expect("Didn't receive data");

        let msg = paxos_decode(&recvbuf, bytes_n);

        println!("bufrecv: {:?}", msg);
        

        
        let msg = paxos_encode(&[1,2,3]);
        match s.send_to(&msg, cfg.get("acceptors").unwrap()) {
            Ok(bytes_sent) => println!("sent {} bytes", bytes_sent),
            Err(e) => panic!("couldn't send from proposer, err: {}", e)
        }
        stdout().flush().unwrap();
        return;
        
    }
}

fn client(cfg: HashMap<String, SocketAddrV4>, id: u16) {
    println!("> client {}", id);
    let s = mcast_sender();

    loop {
        let mut val = String::new();
        let stdin = stdin(); // We get `Stdin` here.
        
        match stdin.read_line(&mut val) {
            Ok(_) => {
                // try to parse val as integer
                match val.parse::<i32>() {
                    Ok(v) => {
                        let msg = paxos_encode(&[v]);
                        s.send_to(&msg, cfg.get("proposers").unwrap());
                    },
                    Err(_) => panic!("value {} is not an integer", val),
                }
            },
            Err(e) => panic!("failed to read stdin. Error: {}", e)
        }
    
    }
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
    

    match role {
        "acceptor" => acceptor(cfg, id),
        "learner" => learner(cfg, id),
        "client" => client(cfg, id),
        "proposer" => proposer(cfg, id),
        _ => println!("Invalid role: {}", role)

    }
}
