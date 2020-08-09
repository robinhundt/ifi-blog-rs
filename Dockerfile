# Our first FROM statement declares the build environment.
FROM rust:latest AS builder

# Add our source code.
COPY ./ ./
# Build our application.
RUN cargo build --release

RUN mkdir -p /build-out
RUN cp target/release/ifi-blog-rs /build-out


# Now, we need to build our _real_ Docker container, copying in `using-diesel`.
FROM debain:latest

ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get -y install ca-certificates libssl-dev && rm -rf /var/lib/apt/lists/*

COPY --from=builder \
    /build-out/ifi-blog-rs \
    /usr/local/bin/
CMD /usr/local/bin/ifi-blog-rs
