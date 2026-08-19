# Swift console example

This macOS example consumes the generated Swift package and performs one
connection, ping, and graceful shutdown. It does not require a running game or
data source.

Run it from the repository root on macOS with Xcode and all Rust targets
configured by `boltffi.toml` installed:

```bash
just example swift
```

The runner packages the binding, starts a temporary service, supplies its
endpoint ticket, and shuts it down afterwards. The bindings workflow performs
the same exchange automatically on macOS.
