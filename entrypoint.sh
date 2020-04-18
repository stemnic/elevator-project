#!/bin/sh

service ssh start

su elev -c "tmux new-session -d -s elevator '/usr/src/elevator-project/SimElevatorServer'"
su elev -c "tmux splitw -h -p 66 -d -t elevator 'export RUST_BACKTRACE=1; while true; do elevator-project; echo Server crashed restarting in 5 sec; sleep 5; done'"
su elev -c "tmux splitw -h -p 50 -d -t elevator '/usr/src/elevator-project/network_stress_test.sh'"
su elev -c "tmux set -g mouse on"
su elev -c "tmux set-option remain-on-exit on"

tail -f /dev/null