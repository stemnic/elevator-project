#!/bin/sh

sudo iptables -A INPUT -p udp --dport 26665 -m statistic --mode random --probability 0.2 -j DROP
sudo iptables -A INPUT -p udp --sport 26665 -m statistic --mode random --probability 0.2 -j DROP