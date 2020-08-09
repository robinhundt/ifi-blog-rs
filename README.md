# Ifi-Blog-Rs

This is a small telegram bot used to get updates from the Göttingen cs deanery [blog](https://blog.stud.uni-goettingen.de/informatikstudiendekanat/) in various Telegram groups and channels.

## Requirements
You need the latest stable Rust toolchain, which can be installed via [rustup](https://rustup.rs).  
This path to the SQLite database file must be stored in the `DATABASE_URL` env variable in the format `sqlite:<path>`. This variable must also be present at build time.  
Furthermore a Telegram bot token must be stored in the `TELOXIDE_TOKEN` env variable and the name of the bot in `BOT_NAME`. These do not need to be available at build time.

## Building

```
cargo build --release
```

## Running

```
export DATABASE_URL=sqlite:<PATH>
export TELOXIDE_TOKEN=<token>
export BOT_NAME=<name without @>
./target/release/ifi-blog-rs
```

