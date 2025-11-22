FROM python:3

WORKDIR /usr/src/app

RUN pip install numpy matplotlib scipy

RUN apt-get update && apt-get install -y rustup

RUN rustup default stable

COPY . .

RUN cargo build --release

EXPOSE 4096

CMD [ "/usr/src/app/target/release/python_tool_executor" ]
