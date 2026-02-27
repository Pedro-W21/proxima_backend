FROM debian:latest

WORKDIR /usr/src/proxima

RUN apt-get update && apt-get install -y rustup gcc pkg-config libssl-dev

RUN rustup default nightly

COPY . .

WORKDIR /usr/src/proxima/proxima_backend_server/

RUN cargo build --release

ENTRYPOINT [ "/usr/src/proxima/proxima_backend_server/target/release/proxima_backend_server", "/etc/proxima/configuration.yml" ]
