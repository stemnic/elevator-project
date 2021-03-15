#!/bin/sh

service ssh start

su elev -c "tmux new-session -d -s elevator 'while true; do /usr/src/elevator-project/SimElevatorServer; echo ElevServer crashed restarting in 5 sec; sleep 5; clear; done'"
su elev -c "tmux splitw -h -p 50 -d -t elevator '/bin/bash'"
su elev -c "tmux splitw -h -p 66 -d -t elevator 'export RUST_BACKTRACE=1; while true; do elevator-project; echo Server crashed restarting in 5 sec; sleep 5; done'"
su elev -c "tmux set -g mouse on"
su elev -c "tmux set-option remain-on-exit on"

tail -f /dev/null