"""Shared HTTPS plumbing for the deployment scripts.

Two failures here have nothing to do with feeds, prices or Solana, and both are
routinely misread as an outage: a machine that cannot verify certificates, and
a proxy that refuses the host outright. This is the one place they are handled,
so every script reports them the same way and says what to actually do.
"""

from __future__ import annotations

import os
import ssl
from pathlib import Path


def tls_context() -> ssl.SSLContext:
    """A context that trusts what this machine actually has.

    Python does not share the OS trust store on every platform. A python.org
    build on macOS ships its own empty one, which fails every HTTPS request with
    "unable to get local issuer certificate" -- an error that reads like a
    server problem and is not one. Certifi, then an explicitly configured
    bundle, then the OS default; the first that exists wins.
    """
    for var in ("SSL_CERT_FILE", "REQUESTS_CA_BUNDLE"):
        path = os.environ.get(var)
        if path and Path(path).exists():
            return ssl.create_default_context(cafile=path)
    try:
        import certifi
        return ssl.create_default_context(cafile=certifi.where())
    except ImportError:
        return ssl.create_default_context()


def diagnose(exc: BaseException) -> str | None:
    """Turn the two network failures that are not about feed ids into advice.

    Both otherwise surface as a raw urllib traceback that says nothing about
    what to do, and neither is fixed by trying a different ticker.
    """
    text = str(exc)
    if "CERTIFICATE_VERIFY_FAILED" in text:
        return (
            "This machine cannot verify TLS certificates -- it is a local trust\n"
            "store problem, not a Hermes outage. Pick one:\n"
            "  macOS, python.org build:  /Applications/Python\\ 3.x/Install\\ Certificates.command\n"
            "  any platform:             python3 -m pip install certifi\n"
            "  explicit bundle:          export SSL_CERT_FILE=/path/to/ca-bundle.crt\n"
            "Then re-run. To proceed without network access at all, see --from-file."
        )
    if "403" in text and ("Tunnel" in text or "proxy" in text.lower()):
        return (
            "An HTTPS proxy refused to connect to Hermes (403 on CONNECT), so this\n"
            "network does not allow the host. That is a policy decision, not a bug.\n"
            "Fetch the feed ids somewhere with access and pass them in:\n"
            "  curl -s 'https://hermes.pyth.network/v2/price_feeds?query=AAPL' > aapl.json\n"
            "  python3 scripts/fetch-feed-ids.py --from-file aapl.json ..."
        )
    return None
