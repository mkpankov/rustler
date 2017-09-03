FROM centos:7

ENV HOME=/root

WORKDIR $HOME

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain nightly

ENV PATH=${PATH}:$HOME/.cargo/bin/

RUN yum install -y gcc make unzip

WORKDIR rustler

COPY Cargo.toml Cargo.lock ./

COPY src/dummy.rs src/dummy.rs

RUN cargo build --release --lib

COPY src src

RUN cargo build --release

RUN cp target/release/rustler .

RUN cargo clean

EXPOSE 80

ENV ROCKET_ENV=prod

ENV ROCKET_WORKERS=4000

ENV RUST_BACKTRACE=1

ENV ENVIRONMENT=prod

CMD ./rustler
