![Rust](https://github.com/stemnic/elevator-project/workflows/Rust/badge.svg)
# Elevator project for TTK4145 in Rust

This program conists of the main program elevator-project which controlles a single elevator. It will cooperate with different elevator "servers" which are broadcasting and listening on the same port on the network.
It can be theoretically scaled dynamically to match desired number of floors and communicating elevators.

## Use
```
elevator-project (elevator id) (udp_broadcast_port) (elevator hardware ip) (elevator hardware port)
```

All elevators on the network should have different id's

## Dependencies
- [Elevator-driver](https://github.com/stemnic/elevator-driver) a library for interfacing with the physical elevator tcp interface
- [network-rust](https://github.com/stemnic/network-rust) a library for peer to peer communication and udp broadcast messaging

## Docker Setup
To make it easier to test with multiple elevators on the same machine a Docker setup has been made where the number of elevators can be dynamically set.

Each elevator (server and simulator) lives in it's own container and are connected to a display container over ssh.
### Prerequisite
- [Docker](https://docs.docker.com/install/)
- [Docker-compose](https://docs.docker.com/compose/install/)

### Running the server
Defaults to 3 elevators
```
docker-compose build
docker-compose up
```
`docker-compose build` needs to be run every time there is a change to the source files. 
Optionally you can specify the number of elevators you want
```
docker-compose up --scale elevator=(number)
```
After a while you will be asked to enter the tmux session in the display container with the following command
```
docker exec -ti elevDisplay bash -c "tmux a -t elevs"
```

### Shutting down
Simply use `Ctrl+C` or `docker-compose down` in another terminal in the project directory
