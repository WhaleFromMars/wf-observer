# Examples

Examples are grouped by language and then by framework or application type.

- `rust/dioxus` demonstrates the full API.
- The console examples exercise the minimal FFI bindings by connecting,
  pinging, and shutting down.

Run one or more console examples from the repository root with
[Just](https://just.systems/). The runner packages their bindings and manages a
temporary service automatically:

```bash
just example python csharp java kotlin
```

The Swift example requires macOS and can be run with `just example swift`.
