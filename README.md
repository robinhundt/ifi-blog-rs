# Ifi-Blog-Rs

This is a small telegram bot used to get updates from the Göttingen cs deanery [blog](https://blog.stud.uni-goettingen.de/informatikstudiendekanat/) in various Telegram groups and channels.

## Requirements
You need the latest stable Rust toolchain, which can be installed via [rustup](https://rustup.rs).  
Additionally a `Postgres` instance needs to be running and reachable under `postgres://postgres:password@localhost:5432` during development. This URL mus be available in the `DATABASE_URL` env variable.  
Furthermore a Telegram bot token must be stored in the `TELOXIDE_TOKEN` env variable and the name of the bot in `BOT_NAME`.

## Building

```
cargo build --release
```

## Running

For development purposes, you can start a postgres via docker with:
```
docker run --name postgres --rm -e POSTGRES_PASSWORD=password -p 5432:5432 postgres
```

```
export DATABASE_URL=<URL>
export TELOXIDE_TOKEN=<token>
export BOT_NAME=<name without @>
./target/release/ifi-blog-rs
```

