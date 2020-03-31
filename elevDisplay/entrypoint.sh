#!/bin/sh

echo "Waiting 3 seconds for elevators to come online"
subnetprefix=`hostname -I | grep -P "^\d+\.\d+\.\d+" -o`
sleep 3
num=`nmap -sS -p 22 --open $subnetprefix.0/24 -oG - | grep 22 | grep Host | grep -oP "\d+\.\d+\.\d+\.\d+" | wc -l`
echo "Discovered $num elevators, connecting..."
num=$(( $num + 0 ))
iter=$num
for ip in `nmap -sS -p 22 --open $subnetprefix.0/24 -oG - | grep 22 | grep Host | grep -oP "\d+\.\d+\.\d+\.\d+"`; do if [ $iter -eq $num ]; then tmux new-session -d -s elevs "sshpass -p pass ssh -t -o \"StrictHostKeyChecking=no\" elev@$ip 'tmux a'"; else tmux splitw -p $((100/$num)) -d -t elevs "sshpass -p pass ssh -t -o \"StrictHostKeyChecking=no\" elev@$ip 'tmux a'"; fi; iter=$(($iter - 1)); done
tmux select-layout -t elevs even-vertical
tmux set -g mouse on
echo Display server started. Type 'docker exec -ti elevDisplay bash -c "tmux a -t elevs"' to enter
tail -f /dev/null