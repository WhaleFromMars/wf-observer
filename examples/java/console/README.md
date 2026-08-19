# Java console example

This example consumes BoltFFI's generated Java sources and packaged JNI
library. It performs one connection, ping, and graceful shutdown without a
running game or data source.

Run it from the repository root with JDK 17 and Clang installed:

```bash
just example java
```

The runner packages the binding, starts a temporary service, supplies its
endpoint ticket, selects the platform's Gradle wrapper, and shuts the service
down afterwards. The bindings workflow performs the same exchange
automatically.
