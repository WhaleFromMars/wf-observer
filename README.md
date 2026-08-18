# Warframe Observer

A local service (currently foreground, the user must start it) that reads memory from a locally running Warframe process on
Linux and Windows and makes the data available to other applications.

We only read memory. We never write to it or inject code.

## Running the example integration

```bash
# be at the root of the repo
# start the local service
cargo run -p local-service -- run
# then in a separate terminal run the showcase
cargo run -p example-rust-dioxus --features dioxus/desktop
# launch the game at any point
```

Foreign-language clients are generated from `wf_observer_ffi` by
[BoltFFI](https://www.boltffi.dev/). Swift, Java, C#, Python, and browser
TypeScript are configured in `boltffi.toml`. Android bindings are currently
unsupported and disabled.

See the [examples](examples/README.md) for the Dioxus showcase and
minimal generated-binding consumers.

## Profiling

[Hotpath](https://hotpath.rs) profiling is opt-in.
CPU sampling is unavailable on Windows; omit `hotpath/hotpath-cpu` there.

```bash
cargo run -p local-service --features hotpath/hotpath,hotpath/hotpath-alloc,hotpath/hotpath-cpu -- run
```

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this project by you shall be dual licensed as above, without
any additional terms or conditions.
