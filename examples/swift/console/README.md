# Swift console example

This macOS example consumes the generated Swift package and performs one
connection, ping, and graceful shutdown. It does not require a running game or
data source.

From the repository root on macOS, package the Apple binding with Xcode and all
Rust targets configured by `boltffi.toml` installed:

```bash
boltffi pack apple --deny-skipped
```

Start the service in a separate terminal, then pass its endpoint ticket to the
example:

```bash
cargo run -p local-service -- run --print-ticket
swift run --package-path examples/swift/console WFObserverConsole <endpoint-ticket>
```

Copy the value after `WF_OBSERVER_ENDPOINT_TICKET=` into the Swift command.
The bindings workflow performs this exchange automatically on macOS.
