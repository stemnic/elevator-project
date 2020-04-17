FROM debian:latest

RUN apt-get update && apt-get install -y tmux ssh sshpass nmap locales-all && rm -rf /var/lib/apt/lists/*
COPY entrypoint.sh /
RUN chmod +x entrypoint.sh
WORKDIR /
CMD /entrypoint.sh
