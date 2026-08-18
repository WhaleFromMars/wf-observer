# Python console example

This example installs the generated Python wheel and verifies a connection to
the real WF Observer service. It only sends `Ping`, so no game or data source is
required.

Package the binding from the repository root with Python 3.10 or newer:

```bash
boltffi pack python --deny-skipped --python python
python -m venv .venv
# Activate .venv using the command for your shell.
python -m pip install dist/python/wheelhouse/<wheel-file>.whl
```

Start the service in a separate terminal and pass its live endpoint ticket to
the example:

```bash
cargo run -p local-service -- run --print-ticket
python examples/python/console/main.py <endpoint-ticket>
```

Copy the value after `WF_OBSERVER_ENDPOINT_TICKET=` into the Python command.
The bindings workflow performs this complete exchange automatically.
