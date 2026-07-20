FROM rust:1.96.1

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
COPY nmrs ./nmrs

RUN cargo build --locked -p nmrs --release && cargo build --locked -p nmrs

CMD ["/bin/bash"]
