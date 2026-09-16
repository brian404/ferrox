FROM rust:1-slim AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN useradd --system --no-create-home ferrox
WORKDIR /app
COPY --from=builder /app/target/release/ferrox ./ferrox
COPY public ./public
RUN chown -R ferrox:ferrox /app
USER ferrox
EXPOSE 3000
CMD ["./ferrox"]
