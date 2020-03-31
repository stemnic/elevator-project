#!/bin/sh

service ssh start

su elev -c "tmux new-session -d -s elevator '/usr/src/elevator-project/SimElevatorServer'"
su elev -c "tmux splitw -h -p 66 -d -t elevator 'elevator-project'"
su elev -c "tmux set -g mouse on"

tail -f /dev/null