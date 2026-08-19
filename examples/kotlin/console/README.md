# Kotlin/JVM console example

This example consumes the same generated Java/JNI binding as the Java example;
there is no handwritten Kotlin wrapper. It performs one connection, ping, and
graceful shutdown without a running game or data source.

Run it from the repository root with JDK 17 and Clang installed:

```bash
just example kotlin
```

The runner packages the shared Java binding, starts a temporary service,
supplies its endpoint ticket, selects the platform's Gradle wrapper, and shuts
the service down afterwards. The bindings workflow performs the same exchange
automatically.
