"""Typed capability seam: the sidecar never reaches a provider directly."""

from __future__ import annotations

from typing import Any, Protocol
from uuid import uuid4

from aiohttp import ClientSession, ClientTimeout

CAPABILITY_PROTOCOL = "story-capability-request/v1"


class Capability(Protocol):
    async def generate(
        self, route: str, system: str, prompt: str
    ) -> tuple[dict[str, Any], dict[str, Any], str]: ...

    async def validate_package(
        self, package: dict[str, Any], expected_episodes: int
    ) -> dict[str, Any]: ...


class ReviewRejected(RuntimeError):
    """The fail-closed final reviewer refused the package.

    Distinguished from infrastructure failures on purpose: the workflow behaved
    correctly and the story did not pass. Retrying without changing the story
    reproduces it exactly.
    """


class PackageValidationFailed(RuntimeError):
    """The package did not satisfy the artifact contract (schema, episode count).

    A defect in generation or in the contract, not a provider problem.
    """


class RustCapabilityClient:
    def __init__(self, endpoint: str, token: str, timeout_seconds: float = 300.0) -> None:
        self._endpoint = endpoint.rstrip("/")
        self._token = token
        self._timeout = ClientTimeout(total=timeout_seconds)
        self._session: ClientSession | None = None

    async def _call(self, payload: dict[str, Any]) -> dict[str, Any]:
        if self._session is None:
            self._session = ClientSession(timeout=self._timeout)
        async with self._session.post(
            f"{self._endpoint}/v1/capabilities",
            json=payload,
            headers={"Authorization": f"Bearer {self._token}"},
        ) as response:
            if response.status != 200:
                raise RuntimeError(f"Rust capability rejected request: HTTP {response.status}")
            result = await response.json()
        if result.get("schema") != "story-capability-response/v1" or result.get("status") != "ok":
            raise RuntimeError("Rust capability returned an invalid response")
        return result

    async def generate(
        self, route: str, system: str, prompt: str
    ) -> tuple[dict[str, Any], dict[str, Any], str]:
        result = await self._call(
            {
                "schema": CAPABILITY_PROTOCOL,
                "capability": "generate_structured_text",
                "request_id": f"cap_{uuid4().hex}",
                "route": route,
                "system": system,
                "prompt": prompt,
            }
        )
        artifact = result.get("artifact")
        if not isinstance(artifact, dict):
            raise RuntimeError("structured generation did not return an object")
        return artifact, result.get("usage", {}), str(result.get("model", "unknown"))

    async def validate_package(
        self, package: dict[str, Any], expected_episodes: int
    ) -> dict[str, Any]:
        return await self._call(
            {
                "schema": CAPABILITY_PROTOCOL,
                "capability": "validate_artifact",
                "request_id": f"cap_{uuid4().hex}",
                "artifact_schema": "story-package/v1",
                "artifact": package,
                "expected_episodes": expected_episodes,
            }
        )

    async def close(self) -> None:
        if self._session is not None:
            await self._session.close()
            self._session = None
