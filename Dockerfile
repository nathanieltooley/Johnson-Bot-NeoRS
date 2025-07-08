FROM rust:1.88 as builder
WORKDIR /usr/local/src/


# Build dependencies
RUN apt-get update
RUN apt-get install -y pkg-config
RUN apt-get install -y cmake
RUN apt-get install -y libssl-dev

# Copy project into /usr/local/src/
COPY . .

RUN cargo install --path .

FROM debian:bookworm-slim
WORKDIR /usr/local/src/

RUN apt-get update

# Runtime dependencies
RUN apt-get install -y --fix-missing libssl-dev

COPY --from=builder /usr/local/cargo/bin/johnson-nrs /usr/local/bin/johnson-nrs
COPY --from=builder /usr/local/src/resources/ ./resources/
COPY --from=builder /usr/local/src/cfg/ ./cfg/

CMD ["johnson-nrs"]
