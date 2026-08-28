"""Refuse requests to anything but public HTTPS endpoints.

Endpoints come from tracked operator configuration (`eval/judges.json`,
generator config files), never from model output. This guard exists so a
stray config edit — or a future feature that lets a model influence a URL —
cannot turn the evaluation tooling into a probe of internal networks.

Every HTTP egress in `eval/tools` must pass `assert_public_https_endpoint`
before building the request.

Usage:
    from endpoint_guard import assert_public_https_endpoint
    assert_public_https_endpoint(route["endpoint"])
"""

from __future__ import annotations

import ipaddress
import socket
from functools import lru_cache
from urllib.parse import urlparse

_FORBIDDEN_HOSTNAMES = {"localhost", "metadata.google.internal"}


@lru_cache(maxsize=64)
def _resolved_addresses(host: str) -> tuple[str, ...]:
    """Resolve once per host; a private answer anywhere refuses the host."""
    infos = socket.getaddrinfo(host, None)
    return tuple(info[4][0] for info in infos)


def assert_public_https_endpoint(url: str) -> None:
    parsed = urlparse(url)
    if parsed.scheme != "https":
        raise ValueError(f"refusing non-https endpoint: {url}")
    host = (parsed.hostname or "").strip().rstrip(".")
    if not host:
        raise ValueError(f"endpoint has no host: {url}")
    lowered = host.lower()
    if lowered in _FORBIDDEN_HOSTNAMES or lowered.endswith((".local", ".internal")):
        raise ValueError(f"refusing local endpoint: {url}")

    try:
        ipaddress.ip_address(host)
        candidates = [host]
    except ValueError:
        try:
            candidates = list(_resolved_addresses(host))
        except OSError as error:
            raise ValueError(f"endpoint host does not resolve: {url}") from error

    for candidate in candidates:
        address = ipaddress.ip_address(candidate)
        if not address.is_global:
            raise ValueError(
                f"refusing non-public endpoint address {candidate}: {url}"
            )


def https_exchange(
    url: str,
    method: str,
    headers: dict[str, str],
    body: bytes,
    timeout: int,
) -> bytes:
    """The single HTTP egress for the evaluation tooling.

    Validates the endpoint, then opens an HTTPS connection to exactly the
    validated host and port — there is no re-parse gap between check and
    connect. Non-2xx statuses surface as `urllib.error.HTTPError` so the
    existing retry semantics (429/5xx with Retry-After) keep working.
    """
    import http.client
    import io
    import urllib.error
    from email.message import Message

    assert_public_https_endpoint(url)
    parsed = urlparse(url)
    target = parsed.path or "/"
    if parsed.query:
        target += f"?{parsed.query}"
    connection = http.client.HTTPSConnection(
        parsed.hostname,
        parsed.port or 443,
        timeout=timeout,
    )
    try:
        connection.request(method, target, body=body, headers=headers)
        response = connection.getresponse()
        payload = response.read()
        if response.status >= 400:
            header_message = Message()
            for name, value in response.getheaders():
                header_message[name] = value
            raise urllib.error.HTTPError(
                url, response.status, response.reason, header_message, io.BytesIO(payload)
            )
        return payload
    finally:
        connection.close()
