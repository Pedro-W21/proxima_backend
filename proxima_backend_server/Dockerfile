FROM debian:latest

WORKDIR /usr/src/proxima

RUN apt-get update && apt-get install -y rustup gcc pkg-config libssl-dev curl

RUN rustup default nightly

COPY . .

WORKDIR /usr/src/proxima/proxima_backend_server/

RUN curl -L "https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F7734/pdfium-linux-x64.tgz" \ | tar -xz --strip-components=1 lib/libpdfium.so

RUN cargo build --release

ENTRYPOINT [ "/usr/src/proxima/proxima_backend_server/target/release/proxima_backend_server", "/etc/proxima/configuration.yml" ]
