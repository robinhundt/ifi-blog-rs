# Our first FROM statement declares the build environment.
FROM ekidd/rust-musl-builder:latest AS builder

# Add our source code.
ADD --chown=rust:rust . ./

# Build our application.
RUN cargo build --release

# Now, we need to build our _real_ Docker container, copying in `using-diesel`.
FROM alpine:latest
RUN apk --no-cache add ca-certificates
COPY --from=builder \
    /home/rust/src/target/x86_64-unknown-linux-musl/release/ifi-blog-rs \
    /usr/local/bin/
CMD /usr/local/bin/ifi-blog-rs