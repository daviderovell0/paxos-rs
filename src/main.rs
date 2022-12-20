
use std::env;
use std::io::{Error, BufReader, BufRead};
use std::fs::File;
use std::path::Path;
use std::collections::*;

// CONSTANTS
const CONFIG_PATH: &str = "paxos.conf"; // wrt target/<dir>

// AUX functions
fn parse_cfg() -> Result<HashMap<String, String>, Error> {

    let mut cfg = HashMap::new();
    // Create a path to the desired file
    let path = Path::new(CONFIG_PATH);
    println!("{}", path.display());
    // Open the path in read-only mode, returns `io::Result<File>`
    let file = File::open(&path)?;
    let lines = BufReader::new(file).lines(); //read line by line
    
    for line in lines {
        if let Ok(ip) = line {
            let los: Vec<&str> = ip.split(" ").collect();
            cfg.insert(los[0].to_string(), los[1].to_string());
        } //not checking errors
    }
    
    Ok(cfg)
}

fn acceptors() {
    println!("Hello from acceptor");
}

fn proposer() {
    println!("Hello from proposer");
}

fn learner() {
    println!("Hello from learner");
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 3 {
        println!("Usage: paxos-rs <role> <id>");
        return;
    }

    println!("cfg path: {} {}", args[0], args[1]);
    //let cfg = parse_cfg();
    let cfg = match parse_cfg() {
        Ok(h) => h,
        Err(e) => panic!("Failed to parse the configuration file. Err: {}", e),
    };

    println!("{:?}", cfg);
}
