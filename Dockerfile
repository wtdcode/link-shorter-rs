FROM rust:bookworm as builder

RUN mkdir -p /work
WORKDIR /work

COPY src /work/src
COPY Cargo.lock /work/
COPY Cargo.toml /work/

RUN cargo build --release && cp target/release/link-shorter-rs /usr/bin/

FROM rust:bookworm as runner

COPY --from=builder /usr/bin/link-shorter-rs /usr/bin/link-shorter-rs

ENTRYPOINT [ "/usr/bin/link-shorter-rs" ]