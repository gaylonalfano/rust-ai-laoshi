```sh
cargo watch -q -c -x src/ -x "run -q"

```

# Terminal 1 - To run the server

# NOTE: If we change ENV inside .cargo/config.rs,

# the server will auto-restart.

cargo watch -qcw src/ -w .cargo/ -x "run"

# Terminal 2 - To run the quick_dev

cargo watch -qcw examples/ -x "run --example quick_dev"
