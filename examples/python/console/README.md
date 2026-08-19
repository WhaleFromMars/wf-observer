# Python console example

This example installs the generated Python wheel into a temporary environment
and verifies a connection to the real WF Observer service. It only sends
`Ping`, so no game or data source is required.

Run it from the repository root with Python 3.10 or newer:

```bash
just example python
```

The runner packages the binding, starts a temporary service, supplies its
endpoint ticket, and shuts it down afterwards. The bindings workflow performs
the same exchange automatically.
