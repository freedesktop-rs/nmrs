FROM rust:1.96.1

WORKDIR /app

RUN apt-get update && apt-get install -y \
    libdbus-1-dev \
    pkg-config \
    dbus \
    network-manager \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY nmrs ./nmrs

RUN cargo build -p nmrs --release && cargo build -p nmrs

CMD ["/bin/bash"]
