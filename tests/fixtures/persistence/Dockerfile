FROM debian:bookworm-slim
RUN groupadd --gid 1000 fixture \
    && useradd --uid 1000 --gid 1000 --create-home --shell /bin/sh fixture
ENV HOME=/home/fixture
