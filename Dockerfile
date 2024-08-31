FROM rust:1.79-slim-bullseye
WORKDIR /usr/local/src/

# Copy project into /usr/local/src/
COPY . .

RUN apt-get update
# RUN apt-get install -y curl

# RUN curl -L https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux -o /usr/local/bin/yt-dlp
# RUN chmod a+rx /usr/local/bin/yt-dlp

# I dont want to download python just for yt-dlp
# however, the standalone binaries would not cooperate with me
# TODO: Figure out how to standalone yt-dlp binaries
RUN apt-get install -y python3
RUN apt-get install -y python3-pip
RUN pip3 install yt-dlp
RUN yt-dlp -h

RUN apt-get install -y pkg-config
RUN apt-get install -y cmake
RUN apt-get install -y libssl-dev
RUN apt-get install -y --fix-missing libopus-dev
RUN apt-get install -y --fix-missing ffmpeg
RUN cargo build --release

CMD ["cargo", "run", "--release"]

