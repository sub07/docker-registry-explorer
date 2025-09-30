FROM ubuntu:24.04
RUN apt-get update && apt-get install -y curl && rm -rf /var/lib/apt/lists/*
ENV LISTEN_ADDR=0.0.0.0
ENV LISTEN_PORT=80
ENV STATIC_DIR=/app/static
WORKDIR /app
COPY /target/release/docker-registry-explorer /usr/local/bin/docker-registry-explorer
COPY /static/ /app/static/
CMD ["docker-registry-explorer"]
