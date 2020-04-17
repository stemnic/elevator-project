#!/bin/bash

# while-menu-dialog: a menu driven system information program

read -p "Press enter to start network_stress_test";

DIALOG_CANCEL=1
DIALOG_ESC=255
HEIGHT=0
WIDTH=0

display_result() {
  dialog --title "$1" \
    --no-collapse \
    --msgbox "$result" 0 0
}

while true; do
  exec 3>&1
  selection=$(dialog \
    --backtitle "System Information" \
    --title "Menu" \
    --clear \
    --nocancel \
    --menu "Please select:" $HEIGHT $WIDTH 4 \
    "1" "Start blocking UDP traffic between elevators" \
    "2" "Start intermittent 20% packetloss" \
    "3" "Stop all tests" \
    2>&1 1>&3)
  exit_status=$?
  exec 3>&-
  case $exit_status in
    $DIALOG_CANCEL)
      clear
      echo "Program terminated."
      exit
      ;;
    $DIALOG_ESC)
      clear
      echo "Program aborted." >&2
      exit 1
      ;;
  esac
  case $selection in
    0 )
      clear
      echo "Program terminated."
      ;;
    1 )
      result=$(sudo iptables -A INPUT -p udp --sport 26665 -j ACCEPT; sudo iptables -A INPUT -p udp--dport 26665 -j ACCEPT; \
      sudo iptables -A INPUT -p tcp --dport 22 -m conntrack --ctstate NEW,ESTABLISHED -j ACCEPT; \
      sudo iptables -A INPUT -p tcp --dport 15657 -j ACCEPT; sudo iptables -A INPUT -p tcp --sport 15657 -j ACCEPT; \
      sudo iptables -A INPUT -j DROP; \
      echo "Done")
      display_result "Started blocking udp 26665"
      ;;
    2 )
      result=$(sudo iptables -A INPUT -p udp --dport 26665 -m statistic --mode random --probability 0.2 -j DROP; \
      sudo iptables -A INPUT -p udp --sport 26665 -m statistic --mode random --probability 0.2 -j DROP)
      display_result "Starting 20% udp package dropping 26665"
      ;;
    3 )
      result=$(sudo iptables -F)
      display_result "Stop all testing"
      ;;
  esac
done