# paxos-rs
Atomic broadcast with Paxos in Rust.

## Info
This program implements a distributed programming primitive
called Atomic Broadcast using the Paxos [1] algorithm in the 
[Rust](https://www.rust-lang.org) programming language. The program 
uses IP multicast as communcation method between different processes, 
each one of the Paxos "actors". It can be run on a single machine.

Paxos is implemented with the optimisation where 2B messages are sent
directly to the learner. A timeout is used to restart incomplete instances
where message loss occurred.  

## Testing

- Install [Rust](https://www.rust-lang.org/tools/install).
- Clone repository and compile:
```sh
git clone https://github.com/daviderovell0/paxos-rs && cd paxos-rs
cargo build
```
- Run tests via the scripts:
```sh
tests/run.sh tests <N> # vanilla-run
tests/run_loss.sh tests <N> # n
```
The script will generate `N` integers, feed them to the client processes 
and send them to the Paxos proposers, starting the paxos instance.

Each Paxos process (proposer, acceptor, learner) is started separately
via the corresponding scripts in `tests`.

Debugging info and message being sent are written to `stdout`. 
Values proposed and learned are written to `prop<n>` and `learn<n>` files
respectively.

## Evaluation and limitations
All values proposed are learned correctly in the vanilla run for an arbitrary 
number of values proposed. To test with more than 1000 values increase the last 
sleep timeout in the testing scripts.

When message losses occur, messages that are seen by the proposer are learned most
of the time. Note that messages that fail to reach the proposer (sent by clients and
lost) are not considered by Paxos. Current limitation is that messages that are lost 
after phase 2A is sent (proposer-> acceptor) might not be recovered, as the restart 
thread monitor only the proposer.

No learner-catch up mechanism is implemented.

## Credits
This program was developed as possible solution for the final assignment 
of the Distributed Algorithms 2022 course at [USI](usi.ch) held by professor 
[Fernando Pedone](https://www.inf.usi.ch/faculty/pedone/).

## References
- [1] Lamport, Leslie. “Paxos Made Simple.” (2001). [link](https://lamport.azurewebsites.net/pubs/paxos-simple.pdf)
