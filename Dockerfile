FROM rust:1.100.0

WORKDIR /app

RUN apt-get update && apt-get install -y \
    libdbus-1-dev \
    pkg-config \
    dbus \
    dnsmasq-base \
    ethtool \
    hostapd \
    iproute2 \
    iw \
    network-manager \
    wpasupplicant \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY examples ./examples
COPY tests ./tests

RUN cargo build --locked --release && cargo build --locked

CMD ["/bin/bash"]
