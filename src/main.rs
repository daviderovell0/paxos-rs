//! Atomic commit using Paxos. Uses the optimisation where
//! 2B messages are sent directly to the learners.
//! 
//! # Message structure:
//! Each message exchanged is a list of [i32] (signed 32-bit integers) arranged
//! as follows:
//! 
//! `[instance number][phase ID]<[1][2]...[n]>`
//! 
//! where `<...>` is the payload depending on the phase:
//! - 1A: `[c-rnd]`
//! - 1B: `[rnd][v-rnd][v-val]`
//! - 2A: `[c-rnd][c-val]`
//! - 2B: `[v-rnd][v-val]`
//! 

use std::time::Duration;
use std::{env, thread};
use std::io::{Error, BufReader, BufRead, stdout, stdin, Write};
use std::fs::File;
use std::collections::*;
use std::net::{UdpSocket, Ipv4Addr, SocketAddrV4};
use socket2::*;
use std::mem::MaybeUninit;
use std::sync::mpsc::{self, Receiver};

// CONSTANTS
const CONFIG_PATH: &str = "paxos.conf"; // wrt where the program is run. assuming home
const QUORUM: i32 = 2;
const TIMEOUT: u64 = 500; // in ms

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

fn proposer_timeout(rx: Receiver<i32>, cfg: HashMap<String, SocketAddrV4>) {
    let mut instances = Vec::<i32>::new();
        let thread_socket = mcast_sender();
        let mut round = 0;
        loop {
            
            match rx.try_recv() { // read instances that reached a quorum
                Ok(instance) => {
                    if !instances.contains(&instance) { // could have duplicates
                        instances.push(instance);
                    }
                }
                // restart incomplete isntances when no more incoming, then timeout
                Err(_) => {
                    let mut printout = String::from("Restarted instances:\n");
                    round += 1; // increase paxos round
                    let mut prev = -1;     
                    instances.sort(); // increasing order
                    for ins in instances.iter() {
                        if ins != &(prev + 1) {
                            // hole found, loop over missing instances
                            for to_restart in prev + 1..*ins {
                                printout.push_str(&format!("{}-",to_restart));
                                // send restart message (id=3) to proposers
                                let outmsg = paxos_encode(&[to_restart, 3, round]);
                                match thread_socket.send_to(&outmsg, cfg.get("proposers").unwrap()) {
                                    Ok(_) => (),
                                    Err(e) => panic!("couldn't send from proposer, err: {}", e)
                                }
                            }
                        }
                        prev = *ins;
                    }
                    // print debug message
                    if !printout.ends_with("\n") {
                        println!("{}", printout);
                    }
                     
                    // wait for timeout and re-check
                    thread::sleep(Duration::from_millis(TIMEOUT));
                }
            }
        }
}

fn proposer(cfg: HashMap<String, SocketAddrV4>, id: u16) {
    println!("> proposer {}", id);
    // init variables
    let s = mcast_sender();
    let r = mcast_receiver(cfg.get("proposers")
    .expect("no entry for key 'proposers' in config file"));
    // c-rnd, c-val, quorum (Q), highest-v-rnd (k) and its associated value (k-val)
    // for every paxos instance indexed by instance number
    let mut states = HashMap::<i32, HashMap<&str, i32>>::new();
    let mut instance_counter = 0;


    // start repeat_paxos thread:
    // restart received instances that do not have a sufficent quorum (< 2A)
    // within a timeout. Cause is message loss
    let (tx, rx) = mpsc::channel();
    let cfg_copy = cfg.clone();
    thread::spawn(move || proposer_timeout(rx, cfg_copy));
    
    loop {

        let mut recvbuf = [MaybeUninit::new(0); 128];
        let (bytes_n, _src_addr) = r.recv_from(&mut recvbuf)
                                    .expect("Didn't receive data");

        let inmsg = paxos_decode(&recvbuf, bytes_n);
        let instance = inmsg[0]; // paxos instance number
        let phase = inmsg[1];

        match phase {

            0 => { // phase 1A: message received from client
                let value = inmsg[2];
                // save new instance
                states.insert(instance_counter,
                HashMap::from([
                    ("c-rnd", 0),
                    ("c-val", value),
                    ("q", 0),
                    ("k", -1),
                    ("k-val", -1)
                ]));
                
                // send 1A
                let outmsg = paxos_encode(&[instance_counter, 1, 0]);
                match s.send_to(&outmsg, cfg.get("acceptors").unwrap()) {
                    Ok(_) => (),//println!("{}-1A | received val: {}", instance_counter, value),
                    Err(e) => panic!("couldn't send from proposer, err: {}", e)
                }
                instance_counter += 1;
            },

            1 => { // phase 2A: received 1B from acceptor
                //println!("recvd: {}", instance);

                match states.get_mut(&instance) {
                    Some(state) => {
                        if state["c-rnd"] >= inmsg[2] {
                            let mut value = state["c-val"];
                            // increase quorum
                            state.insert("q", state["q"] + 1 );
                            // k
                            if inmsg[3] > state["k"] { 
                                state.insert("k", inmsg[3]);
                                state.insert("k-val", inmsg[4]);
                                value = state["k-val"];
                            }
                            
                            if state["q"] >= QUORUM { // if quorum met
                                // println!("quorum reached: {}", state["q"]);
                                // send 2A to acceptors
                                let payload = [instance, 2, state["c-rnd"], value];
                                let outmsg = paxos_encode(&payload);
                                match s.send_to(&outmsg, cfg.get("acceptors").unwrap()) {
                                    Ok(_) => println!("{}-2A | payload: {:?}", instance, &payload),
                                    Err(e) => panic!("couldn't send from proposer, err: {}", e)
                                }

                                // communicate to repeat_paxos thread
                                //println!("sending to thread: {}", instance);
                                tx.send(instance).unwrap();
                            }
                        }
                    },
                    None => panic!("Instance number {} was never proposed", instance)
                }
                },
            3 => { // restart consensus
                match states.get_mut(&instance) {
                    Some(state) => {
                        // update round
                        state.insert("c-rnd", inmsg[2]);
                        // send 1A
                        let outmsg = paxos_encode(&[instance, 1, inmsg[2]]);
                        match s.send_to(&outmsg, cfg.get("acceptors").unwrap()) {
                            Ok(_) => (),//println!("{}-1A | received val: {}", instance_counter, value),
                            Err(e) => panic!("couldn't send from proposer, err: {}", e)
                        }
                        
                    },
                    None => panic!("Instance number {} was never proposed", instance)
                }
                    
                   
            }
            _ => {
                panic!("acceptor {}, phase {} not recognised", id, phase);
            }
        }
        
        stdout().flush().unwrap(); // print
    }
}

fn acceptor(cfg: HashMap<String, SocketAddrV4>, id: u16) {
    println!("> acceptor {}", id);
    let s = mcast_sender();
    let r = mcast_receiver(cfg.get("acceptors")
    .expect("no entry for key 'acceptors' in config file"));
    // rnd, v-rnd, v-val
    // for every paxos instance indexed by instance number
    let mut states = HashMap::<i32, HashMap<&str, i32>>::new();

    loop {
        let mut recvbuf = [MaybeUninit::new(0); 128];
        let (bytes_n, _src_addr) = r.recv_from(&mut recvbuf)
                                    .expect("Didn't receive data");

        let inmsg = paxos_decode(&recvbuf, bytes_n);
        let instance = inmsg[0]; // paxos instance number
        let phase = inmsg[1];
        let init_state = HashMap::from([
            ("rnd", -1),
            ("v-rnd", -1),
            ("v-val", -1)
        ]);

        match phase {
            1 => { // phase 1B: received 1A from proposer
                // get current paxos instance or initialise it if first time
                let state = states.entry(instance)
                .or_insert(init_state);
                
                if inmsg[2] >= state["rnd"] { // inmsg[2] is c-rnd
                    state.insert("rnd", inmsg[2]);

                    // send 1B
                    let payload = [instance, 1, state["rnd"], state["v-rnd"], state["v-val"]];
                    let outmsg = paxos_encode(&payload);
                    match s.send_to(&outmsg, cfg.get("proposers").unwrap()) {
                        Ok(_) => println!("{}-1B | payload: {:?}", instance, payload),
                        Err(e) => panic!("couldn't send from acceptor, err: {}", e)
                    }
                }
            },
            2 => { // phase 2B: received 1A from proposer
                match states.get_mut(&instance) {
                    Some(state) => {
                        if inmsg[2] >= state["rnd"] {
                            state.insert("v-rnd", inmsg[2]);
                            state.insert("v-val", inmsg[3]);

                            //send 2B to learners
                            let payload = [instance, 2, state["v-rnd"], state["v-val"]];
                            let outmsg = paxos_encode(&payload);
                            match s.send_to(&outmsg, cfg.get("learners").unwrap()) {
                                Ok(_) => println!("{}-2B | payload: {:?}", instance, payload),
                                Err(e) => panic!("couldn't send from acceptor, err: {}", e)
                            }
                        }
                    },
                    None => panic!("Instance number {} was never proposed", instance)
                }
            },
            _ => {
                panic!("acceptor {}, phase {} not recognised", id, phase);
            }
            
        }
        stdout().flush().unwrap()
    }
}

fn learner(cfg: HashMap<String, SocketAddrV4>, id: u16) {
    //println!("> learner {}", id);
    //let s = mcast_sender();
    let r = mcast_receiver(cfg.get("learners")
    .expect("no entry for key 'learners' in config file"));
    let mut itl = 0; // instance to learn
    // dict of (v-rnd, v-val, quorum) - indexed by instance
    let mut states = HashMap::<i32, (i32, i32, i32)>::new();
   
    loop {
        let mut recvbuf = [MaybeUninit::new(0); 128];
        let (bytes_n, _src_addr) = r.recv_from(&mut recvbuf)
                                    .expect("Didn't receive data");

        let inmsg = paxos_decode(&recvbuf, bytes_n);
        let instance = inmsg[0]; // paxos instance number
        let phase = inmsg[1];
      
        match phase {
            2 => { // phase 3: received 2B from acceptor
                // skip if we've learned the instance
                if instance < itl { 
                    continue;
                }
                // get quorum for received instance and update the states of the values
                let mut q = match states.get_mut(&instance) {
                    Some(t) => {
                        if inmsg[2] == t.0 { // if v-rnd == previous rounds
                            t.2 += 1; // increase quorum
                            t.2
                        }
                        else if inmsg[2] > t.0 {
                            // should reset current round?
                            t.0 = inmsg[2]; // update with newer round
                            t.1 = inmsg[2]; // corresponding value
                            t.2 = 1; // reset quorum
                            1
                        }
                        else { t.2 } // older round, keep current
                    },
                    None => { // first time we receive the value
                        states.insert(instance, (inmsg[2], inmsg[3], 1));
                        1
                    }
                };

                if instance == itl { 
                    while q >= QUORUM { // learn all values!
                        let val = states[&itl].1; // get value
                        println!("{}",val); // write it
                        states.remove(&itl); // remove instance
                        itl += 1;
                        // get the next value. if empty it means we haven't
                        // seen that particular instance yet
                        q = match states.get(&itl) {
                            Some(t) => t.1,
                            None => 0
                        };
                    }
                }
            },
            _ => panic!("learner {} unkown phase: {}", id, phase)
        }

        stdout().flush().unwrap()
    }
}

fn client(cfg: HashMap<String, SocketAddrV4>, id: u16) {
    println!("> client {}", id);
    let s = mcast_sender();

    loop {
        let mut val = String::new();
        let stdin = stdin();
        //
        match stdin.read_line(&mut val) {
            Ok(_) => {            
                let val = val.trim(); //remove \n
                // try to parse val as integer
                if val.is_empty() {
                    println!("blank line, no more values");
                    break;
                }
                match val.parse::<i32>() {
                    Ok(v) => {
                        // on success, send value to proposers
                        // structure = [null instance, phase, val]
                        let msg = paxos_encode(&[-1, 0, v]);

                        match s.send_to(&msg, cfg.get("proposers").unwrap()) {
                            Ok(_bytes_sent) => println!("client {} sending: {}", id, val),
                            Err(e) => panic!("Failed sending message, err: {}", e)
                        }
                    },
                    Err(_) => panic!("value {} is not an integer", val),
                }
            },
            Err(e) => panic!("failed to read stdin. Error: {}", e)
        }
        thread::sleep(Duration::from_millis(1));
    
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
