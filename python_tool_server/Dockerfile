FROM docker:latest

WORKDIR /usr/src/app

RUN apk update && apk add gcc rustup

RUN rustup-init -y --default-toolchain stable

COPY . .

RUN /root/.cargo/bin/cargo build --release

ENTRYPOINT [ "/usr/src/app/target/release/python_tool_server" ]
