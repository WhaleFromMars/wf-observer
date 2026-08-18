# C# console example

This example consumes the generated NuGet package and performs one complete
connection, ping, and graceful shutdown. It does not require a running game or
data source.

From the repository root, package the binding with the .NET 10 SDK installed:

```bash
boltffi pack csharp --deny-skipped
```

Start the service in a separate terminal, then pass its endpoint ticket to the
example:

```bash
cargo run -p local-service -- run --print-ticket
dotnet run --project examples/csharp/console -- <endpoint-ticket>
```

Copy the value after `WF_OBSERVER_ENDPOINT_TICKET=` into the second command.
The bindings workflow performs this exchange automatically.
