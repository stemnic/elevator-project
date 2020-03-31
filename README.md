# Elevator project for TTK4145
## Docker Setup
To make it easier to test with multiple elevators on the same machine a Docker setup has been made where the number of elevators can be dynamically set.

Each elevator (server and simulator) lives in it's owm container and are connected to a display container over ssh.
### Prerequisite
- [Docker](https://docs.docker.com/install/)
- [Docker-compose](https://docs.docker.com/compose/install/)

### Running the server
Defaults to 3 elevators
```
docker-compose up
```
Optionally you can specify the number of elevators you want
```
docker-compose up --scale elevator=(number)
```
After a while you will be asked to enter the tmux session in the display container

### Shutting down
Simply use `Ctrl+C` or `docker-compose down` in another terminal in the project directory
