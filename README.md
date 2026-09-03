# Warframe Observer

The repository in its current state does not do any of the advertised features, this is merely the baseline to build upon.
The memory reading aspect has been proven in a private repository, with a far richer api than overwolf, but will only be introduced when
the rest of the stack/setup has been proven.

A local service that reads memory from a locally running Warframe process on
Linux and Windows and makes the data available to other applications.

We only read memory. We never write to it or inject code.

You must explicitly run `wf-observer attach` to start it. This requires
Warframe to be open. The agent shuts itself down when Warframe closes. A
foreground diagnostic mode, `wf-observer attach --foreground`, is planned but
is not implemented yet.

Use `wf-observer status` to inspect the running agent, its Warframe process,
version, and Iroh endpoint identifier.
Use `wf-observer stop` to request a shutdown without closing Warframe.

## Running the example integration

```bash
# be at the root of the repo
# start Warframe, then attach the background agent
cargo run -p wf-observer-cli -- attach
# run the showcase
cargo run -p example-rust-dioxus --features dioxus/desktop
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
cargo run -p wf-observer-cli --features hotpath/hotpath,hotpath/hotpath-alloc,hotpath/hotpath-cpu -- attach
```

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this project by you shall be dual licensed as above, without
any additional terms or conditions.
