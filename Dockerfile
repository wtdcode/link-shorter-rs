FROM rust:bookworm

RUN mkdir -p /work
WORKDIR /work

COPY src /work/src
COPY Cargo.lock /work/
COPY Cargo.toml /work/

RUN cargo build --release && cp target/release/link-shorter-rs /usr/bin/

ENTRYPOINT [ "/usr/bin/link-shorter-rs" ]