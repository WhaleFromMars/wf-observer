# C# console example

This example consumes the generated NuGet package and performs one complete
connection, ping, and graceful shutdown. It does not require a running game or
data source.

Run it from the repository root with the .NET 10 SDK installed:

```bash
just example csharp
```

The runner packages the binding, starts a temporary service, supplies its
endpoint ticket, and shuts it down afterwards. The bindings workflow performs
the same exchange automatically.
