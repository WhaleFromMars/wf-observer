"""Connect to a WF Observer service using the generated Python client."""

from __future__ import annotations

import argparse
import asyncio

from wf_observer import connect


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("endpoint", help="WF Observer service endpoint ID or ticket")
    return parser.parse_args()


async def ping(endpoint: str) -> None:
    client = await connect(endpoint)

    try:
        await client.ping()
    finally:
        await client.shutdown()


def main() -> None:
    endpoint = arguments().endpoint
    asyncio.run(ping(endpoint))
    print("WF Observer ping succeeded")


if __name__ == "__main__":
    main()
