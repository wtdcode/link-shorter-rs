FROM rust:bookworm

RUN mkdir -p /work
WORKDIR /work

COPY src /work/src
COPY cargo.lock /work/
COPY cargo.toml /work

RUN cargo build --release && cp target/debug/link-shorter-rs /usr/bin/

ENTRYPOINT [ "/usr/bin/link-shorter-rs" ]