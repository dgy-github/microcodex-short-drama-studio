"""Parsing helpers for stage-0 judge responses."""

from __future__ import annotations

import json
from typing import Any


def parse_content(content: str) -> dict[str, Any]:
    """Parse judge JSON, tolerating markdown fences from relay frontends."""
    text = (content or "").strip()
    if not text:
        raise ValueError("judge returned empty content")
    if text.startswith("```"):
        first_newline = text.find("\n")
        if first_newline != -1:
            text = text[first_newline + 1 :]
        if text.rstrip().endswith("```"):
            text = text.rstrip()[:-3]
    return json.loads(text)
