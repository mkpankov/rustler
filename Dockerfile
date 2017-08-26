FROM centos:7

ENV HOME=/root

WORKDIR $HOME

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain nightly

ENV PATH=${PATH}:$HOME/.cargo/bin/

RUN yum install -y gcc make unzip

WORKDIR rustler

ADD . ./

RUN cargo build --release

EXPOSE 80

ENV ROCKET_ENV=prod

ENV RUST_BACKTRACE=1

ENV ENVIRONMENT=prod

WORKDIR $HOME/rustler

CMD target/release/rustler
