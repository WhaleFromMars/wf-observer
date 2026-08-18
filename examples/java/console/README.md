# Java console example

This example consumes BoltFFI's generated Java sources and packaged JNI
library. It performs one connection, ping, and graceful shutdown without a
running game or data source.

From the repository root, package the binding with JDK 17 and Clang installed:

```bash
boltffi pack java --deny-skipped
```

Start the service in a separate terminal, then pass its endpoint ticket to the
example:

```bash
cargo run -p local-service -- run --print-ticket
bash examples/gradlew -p examples :java:console:run --args="<endpoint-ticket>"
```

On Windows, use `examples\gradlew.bat` instead. Copy the value after
`WF_OBSERVER_ENDPOINT_TICKET=` into the Gradle command. The bindings workflow
performs this exchange automatically.
