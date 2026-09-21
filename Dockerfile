FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY target/release/ferrox /app/ferrox
COPY public ./public
COPY ferrox.conf ./ferrox.conf

EXPOSE 3000

CMD ["./ferrox"]
