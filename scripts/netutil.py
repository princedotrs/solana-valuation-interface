"""Shared HTTPS plumbing for the deployment scripts.

Two failures here have nothing to do with feeds, prices or Solana, and both are
routinely misread as an outage: a machine that cannot verify certificates, and
a proxy that refuses the host outright. This is the one place they are handled,
so every script reports them the same way and says what to actually do.
"""

from __future__ import annotations

import os
import ssl
import sys
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
        if not path:
            continue
        if Path(path).exists():
            return ssl.create_default_context(cafile=path)
        # Saying nothing here is how a typo'd or copy-pasted placeholder path
        # survives: the variable looks set, TLS still fails, and the error
        # names a certificate problem rather than the empty variable causing it.
        print(f"warning: {var} is set to {path!r}, which does not exist -- ignoring it",
              file=sys.stderr)
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
            "  any platform:             python3 -m pip install certifi\n"
            '  certifi you already have: export SSL_CERT_FILE="$(python3 -m certifi)"\n'
            '  find the macOS fixer:     ls /Applications | grep -i python\n'
            '  if it is not there:       python3 scripts/netutil.py --link-certifi\n'
            "Then re-run. To proceed without network access at all, see --from-file."
        )
    if "403" in text and not ("Tunnel" in text or "proxy" in text.lower()):
        return (
            "The server answered 403, so the connection worked and the request was\n"
            "refused. A CDN rejecting the default urllib agent is the usual cause,\n"
            "and no other ticker will behave differently. Fetch it with curl and\n"
            "feed the result in:\n"
            "  curl -s 'https://hermes.pyth.network/v2/price_feeds?query=AAPL' > aapl.json\n"
            "  python3 scripts/fetch-feed-ids.py --from-file aapl.json AAPL"
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


def link_certifi() -> int:
    """Do what the macOS Install Certificates.command does, without needing it.

    A python.org build looks for one specific CA file and ships without it, so
    every HTTPS request fails until something puts a bundle there. The installer
    leaves that to a .command file in /Applications which is easy to move, delete
    or never notice. This links the certifi bundle into the exact path OpenSSL
    checks, so the fix survives new shells and applies to every tool using this
    Python, not just the scripts in this repo.
    """
    import ssl as _ssl

    try:
        import certifi
    except ImportError:
        print("certifi is not installed. Run: python3 -m pip install certifi", file=sys.stderr)
        return 1

    target = _ssl.get_default_verify_paths().openssl_cafile
    source = certifi.where()
    if not target:
        print("This Python reports no default CA file path, so there is nowhere to link.\n"
              'Use the environment variable instead: export SSL_CERT_FILE="$(python3 -m certifi)"',
              file=sys.stderr)
        return 1

    target_path = Path(target)
    print(f"OpenSSL looks for  {target}")
    print(f"certifi bundle is  {source}")

    if target_path.exists() and not target_path.is_symlink():
        print(f"\n{target} already exists and is a real file, not a symlink.")
        print("Refusing to replace it -- something else manages this trust store.")
        print('Use: export SSL_CERT_FILE="$(python3 -m certifi)"', file=sys.stderr)
        return 1

    try:
        target_path.parent.mkdir(parents=True, exist_ok=True)
        if target_path.is_symlink():
            target_path.unlink()
        target_path.symlink_to(source)
    except OSError as exc:
        print(f"\nCould not write the link: {exc}")
        print("That directory is probably root-owned. Either re-run with sudo, or skip")
        print('this entirely: export SSL_CERT_FILE="$(python3 -m certifi)"', file=sys.stderr)
        return 1

    ctx = _ssl.create_default_context()
    loaded = ctx.cert_store_stats().get("x509_ca", 0)
    print(f"\nlinked. The default context now loads {loaded} CA certificates.")
    return 0 if loaded > 0 else 1


if __name__ == "__main__":
    if "--link-certifi" in sys.argv:
        sys.exit(link_certifi())
    print(__doc__)
    print("Usage: python3 scripts/netutil.py --link-certifi")
