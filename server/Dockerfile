# Build image for the server.
#
# The SpacetimeDB SDK pulls in native-tls/openssl-sys (via tokio-tungstenite),
# which needs pkg-config and the OpenSSL headers to compile. rust:slim ships
# neither, so add them here. The *runtime* needs only libssl.so.3, which
# rust:slim already has — so the long-lived `server` containers run on the stock
# image and only the `build` service uses this one.
FROM rust:slim

RUN apt-get update \
 && apt-get install -y --no-install-recommends pkg-config libssl-dev \
 && rm -rf /var/lib/apt/lists/*
