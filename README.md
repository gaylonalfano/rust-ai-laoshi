## TODO

- [] Add event.rs submodule to ais module [ref](https://github.com/rust10x/rust-ai-buddy/blob/main/crates/ai-buddy/src/ais/event.rs)
- [] Add ai-laoshi-core crate event.rs [ref](https://github.com/rust10x/rust-ai-buddy/blob/main/crates/ai-buddy/src/event.rs)

```sh
cargo watch -q -c -x src/ -x "run -q"

```

# Terminal 1 - To run the server

# NOTE: If we change ENV inside .cargo/config.rs,

# the server will auto-restart.

cargo watch -qcw src/ -w .cargo/ -x "run"

# Terminal 2 - To run the quick_dev

cargo watch -qcw examples/ -x "run --example quick_dev"
