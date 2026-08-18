# Android/Kotlin example

Android bindings are currently unsupported, and packaging is disabled in
`boltffi.toml`.

Iroh requires Android-specific JVM and application-context initialization.
Support will remain disabled until the binding owns initialization and the
packaged result has Android instrumentation coverage.

See [Iroh's Android requirements](https://docs.rs/iroh/latest/iroh/endpoint/struct.Endpoint.html#usage-on-android).
