FROM rust:1.79 as builder
WORKDIR /usr/local/src/

# Copy project into /usr/local/src/
COPY . .

# Build dependencies
RUN apt-get update
RUN apt-get install -y pkg-config
RUN apt-get install -y cmake
RUN apt-get install -y libssl-dev

# RUN curl -L https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux -o /usr/local/bin/yt-dlp
# RUN chmod a+rx /usr/local/bin/yt-dlp

RUN cargo install --path .

FROM debian:bullseye-slim

# RUN apt-get update

# Runtime dependencies
# RUN apt-get install -y --fix-missing libopus-dev
# RUN apt-get install -y --fix-missing ffmpeg

COPY --from=builder /usr/local/cargo/bin/johnson-nrs /usr/local/bin/johnson-nrs
# COPY --from=builder /usr/local/bin/yt-dlp /usr/local/bin/yt-dlp

# RUN yt-dlp -h

CMD ["johnson-nrs"]

