FROM rust:1.79-slim-bullseye
WORKDIR /usr/local/src/

# Copy project into /usr/local/src/
COPY . .

RUN apt-get update

RUN apt-get install -y pkg-config
RUN apt-get install -y libssl-dev
RUN apt-get install -y cmake
RUN cargo build

CMD ["cargo", "run"]

