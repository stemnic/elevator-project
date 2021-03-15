#!/bin/sh

#sudo iptables -A INPUT -p udp --sport 26665 -j ACCEPT
#sudo iptables -A INPUT -p udp --dport 26665 -j ACCEPT


sudo iptables -A INPUT -p tcp --dport 22 -m conntrack --ctstate NEW,ESTABLISHED -j ACCEPT
sudo iptables -A INPUT -p tcp --dport 15657 -j ACCEPT
sudo iptables -A INPUT -p tcp --sport 15657 -j ACCEPT
sudo iptables -A INPUT -j DROP