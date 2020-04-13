FROM rust:latest
RUN apt-get update && apt-get install -y tmux ssh wget locales-all && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/elevator-project

RUN yes pass | adduser elev

RUN wget https://github.com/TTK4145/Simulator-v2/releases/download/v1.5/SimElevatorServer && chmod +x SimElevatorServer

COPY src ./src
COPY Cargo.toml .
COPY .tmux.conf /home/elev/

RUN cargo install --path .

COPY entrypoint.sh .
RUN chmod +x entrypoint.sh

CMD ["./entrypoint.sh"]