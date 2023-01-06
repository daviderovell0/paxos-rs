#!/usr/bin/env bash

# Change the loss percentage here
LOSS=0.2

projdir="$1"
#conf=`pwd`/paxosn="$2"
n="$2"

if [[ x$projdir == "x" || x$n == "x" ]]; then
	echo "Usage: $0 <project dir> <number of values per proposer>"
    exit 1
fi

# following line kills processes that have the config file in its cmdline
KILLCMD="pkill -f paxos-rs"

$KILLCMD

cd $projdir

./loss_set.sh $LOSS

./generate.sh $n > ./prop1
./generate.sh $n > ./prop2

echo "starting acceptors..."

./acceptor.sh 1 &
./acceptor.sh 2 &
./acceptor.sh 3 &

sleep 1
echo "starting learners..."

./learner.sh 1 > ./learn1 &
./learner.sh 2 > ./learn2 &

sleep 1
echo "starting proposers..."

./proposer.sh 1 &
./proposer.sh 2 &

echo "waiting to start clients"
sleep 3
echo "starting clients..."

./client.sh 1 < ./prop1 &
#./client.sh 2 < ../prop2 &

sleep 5

$KILLCMD
wait

./loss_unset.sh

cd -
