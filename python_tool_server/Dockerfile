FROM debian:latest

WORKDIR /usr/src/app

RUN apt-get update && apt-get install -y rustup gcc

RUN rustup default stable

COPY . .

RUN cargo build --release

ENTRYPOINT [ "/usr/src/app/target/release/python_tool_server" ]
