from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import init_project


class InitProjectStateTests(unittest.TestCase):
    def test_initialized_state_is_portable_across_checkout_directories(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "renamed-checkout"
            state = root / ".project" / "state.json"
            state.parent.mkdir(parents=True)
            state.write_text(
                json.dumps(
                    {
                        "initialized": True,
                        "project_name": "MicrocodeX Short Drama Studio",
                        "root_name": "original-checkout",
                        "root_fingerprint": "legacy-value",
                    }
                ),
                encoding="utf-8",
            )

            with mock.patch.object(init_project, "ROOT", root), mock.patch.object(
                init_project, "STATE", state
            ):
                self.assertEqual([], init_project.state_errors())

    def test_uninitialized_state_still_fails(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            state = root / "state.json"
            state.write_text(
                json.dumps({"initialized": False, "project_name": ""}),
                encoding="utf-8",
            )

            with mock.patch.object(init_project, "STATE", state):
                self.assertEqual(
                    ["project is not initialized", "project name is missing"],
                    init_project.state_errors(),
                )


if __name__ == "__main__":
    unittest.main()
