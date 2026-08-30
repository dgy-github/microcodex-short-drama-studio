from __future__ import annotations

import json
from typing import Any, Protocol
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen


class MediaProjectRepository(Protocol):
    def append_prompt_revision(self, revision: dict[str, Any]) -> None: ...

    def append_generation_request(self, request: dict[str, Any]) -> None: ...


class RustMediaProjectClient:
    """Authenticated typed client; it exposes no path, shell or provider secret."""

    def __init__(self, endpoint: str, token: str, timeout_seconds: float = 10.0) -> None:
        if not endpoint.startswith("http://127.0.0.1:") and not endpoint.startswith(
            "http://localhost:"
        ):
            raise ValueError("media project capability must be loopback HTTP")
        if len(token) < 32 or timeout_seconds <= 0:
            raise ValueError("media project capability configuration is invalid")
        self._url = endpoint.rstrip("/") + "/v1/media-projects/records"
        self._token = token
        self._timeout = timeout_seconds

    def append_prompt_revision(self, revision: dict[str, Any]) -> None:
        self._call("append_prompt_revision", revision)

    def append_generation_request(self, request: dict[str, Any]) -> None:
        self._call("append_generation_request", request)

    def _call(self, operation: str, record: dict[str, Any]) -> None:
        body = json.dumps(
            {
                "schema": "media-project-capability-request/v1",
                "operation": operation,
                "record": record,
            },
            ensure_ascii=False,
            separators=(",", ":"),
        ).encode("utf-8")
        request = Request(
            self._url,
            data=body,
            method="POST",
            headers={
                "Authorization": f"Bearer {self._token}",
                "Content-Type": "application/json",
            },
        )
        try:
            with urlopen(request, timeout=self._timeout) as response:
                payload = json.loads(response.read())
        except (HTTPError, URLError, TimeoutError, json.JSONDecodeError) as exc:
            raise RuntimeError("media project capability failed") from exc
        if payload != {
            "schema": "media-project-capability-response/v1",
            "status": "stored",
        }:
            raise RuntimeError("media project capability returned an invalid response")
