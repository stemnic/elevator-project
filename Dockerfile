FROM rust:latest
RUN apt-get update && apt-get install -y tmux ssh wget sudo dialog iptables vim locales-all && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/elevator-project

RUN yes pass | adduser elev
RUN adduser elev sudo
RUN echo '%sudo ALL=(ALL) NOPASSWD:ALL' >> /etc/sudoers

RUN wget https://github.com/TTK4145/Simulator-v2/releases/download/v1.5/SimElevatorServer && chmod +x SimElevatorServer

COPY src ./src
COPY Cargo.toml .
COPY .tmux.conf /home/elev/

RUN cargo install --path .

COPY network_stress_test.sh .
RUN chmod +x network_stress_test.sh

COPY net_block.sh .
RUN chmod +x net_block.sh

COPY net_prob_drop.sh .
RUN chmod +x net_prob_drop.sh

COPY entrypoint.sh .
RUN chmod +x entrypoint.sh

CMD ["./entrypoint.sh"]