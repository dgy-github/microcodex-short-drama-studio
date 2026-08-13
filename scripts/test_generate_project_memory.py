from __future__ import annotations

import io
import json
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import generate_project_memory as generator


class GenerateProjectMemoryTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)
        self.root_patch = mock.patch.object(generator, "ROOT", self.root)
        self.memory_patch = mock.patch.object(
            generator, "MEMORY", self.root / "docs" / "project-memory"
        )
        self.root_patch.start()
        self.memory_patch.start()

    def tearDown(self) -> None:
        self.memory_patch.stop()
        self.root_patch.stop()
        self.temporary_directory.cleanup()

    def write_source(self, relative: str, content: str = "const visible = true;\n") -> Path:
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")
        return path

    def test_discover_excludes_generated_dependency_report_and_test_sources(self) -> None:
        included = self.write_source("src/main.ts")
        excluded = [
            ".git/hidden.py",
            ".venv/hidden.py",
            "node_modules/hidden.ts",
            "target/hidden.rs",
            "dist/hidden.js",
            "build/hidden.py",
            "coverage/hidden.ts",
            "playwright-report/hidden.js",
            "test-results/hidden.ts",
            ".workbuddy/hidden.py",
            ".zcode/hidden.py",
            "docs/project-memory/generated.py",
            "tests/unit.py",
            "e2e/browser.ts",
            "apps/desktop/e2e-tauri/runner.ts",
            "src/component.test.ts",
            "src/component.spec.ts",
            "src/test_helper.py",
        ]
        for relative in excluded:
            self.write_source(relative)

        paths = {item["path"] for item in generator.discover()}

        self.assertIn(included.relative_to(self.root).as_posix(), paths)
        self.assertEqual({"src/main.ts"}, paths)

    def test_python_symbols_only_include_module_level_definitions(self) -> None:
        path = self.write_source(
            "package/module.py",
            "class PublicClass:\n"
            "    def method(self):\n"
            "        pass\n"
            "\n"
            "def public_function():\n"
            "    def nested():\n"
            "        pass\n"
            "\n"
            "async def _private_function():\n"
            "    pass\n",
        )

        items = generator.symbols(path)
        by_name = {item["name"]: item for item in items}

        self.assertEqual(
            {"module", "PublicClass", "public_function", "_private_function"},
            set(by_name),
        )
        self.assertTrue(by_name["PublicClass"]["public"])
        self.assertTrue(by_name["public_function"]["public"])
        self.assertFalse(by_name["_private_function"]["public"])
        self.assertFalse(by_name["module"]["public"])

    def test_rust_symbols_keep_private_items_and_mark_pub_items_public(self) -> None:
        path = self.write_source(
            "crate/src/lib.rs",
            "pub struct PublicType;\n"
            "struct PrivateType;\n"
            "impl PublicType {\n"
            "    pub fn method() {}\n"
            "    fn helper() {}\n"
            "}\n",
        )

        by_name = {item["name"]: item for item in generator.symbols(path)}

        self.assertTrue(by_name["PublicType"]["public"])
        self.assertFalse(by_name["PrivateType"]["public"])
        self.assertTrue(by_name["method"]["public"])
        self.assertFalse(by_name["helper"]["public"])

    def test_web_symbols_only_mark_exported_declarations_public(self) -> None:
        path = self.write_source(
            "web/component.ts",
            "export function publicFunction() {}\n"
            "function localFunction() {}\n"
            "export const publicValue = 1;\n"
            "const localValue = 2;\n",
        )

        by_name = {item["name"]: item for item in generator.symbols(path)}

        self.assertTrue(by_name["publicFunction"]["public"])
        self.assertFalse(by_name["localFunction"]["public"])
        self.assertTrue(by_name["publicValue"]["public"])
        self.assertFalse(by_name["localValue"]["public"])

    def test_render_uses_public_flag_for_interface_catalog_and_json(self) -> None:
        items = [
            generator._symbol("Web", "src/main.ts", "module", "main", 1, False),
            generator._symbol("Web", "src/main.ts", "function", "exported", 2, True),
            generator._symbol("Web", "src/main.ts", "function", "local", 3, False),
        ]

        outputs = generator.render(items)
        interface = outputs[generator.MEMORY / "INTERFACE_CATALOG.md"]
        serialized = json.loads(outputs[generator.MEMORY / "SYMBOL_INDEX.json"])

        self.assertIn("`exported`", interface)
        self.assertNotIn("`local`", interface)
        self.assertEqual([False, True, False], [item["public"] for item in serialized])

    def test_replace_retries_then_raises_with_both_paths(self) -> None:
        temporary = self.root / "temporary.tmp"
        target = self.root / "target.md"
        temporary.write_text("content", encoding="utf-8")
        error = PermissionError("locked")

        with mock.patch.object(Path, "replace", side_effect=error) as replace, mock.patch.object(
            generator.time, "sleep"
        ) as sleep:
            with self.assertRaises(OSError) as raised:
                generator._replace_with_retry(temporary, target)

        self.assertEqual(generator.REPLACE_ATTEMPTS, replace.call_count)
        self.assertEqual(generator.REPLACE_ATTEMPTS - 1, sleep.call_count)
        self.assertIn(str(target), str(raised.exception))
        self.assertIn(str(temporary), str(raised.exception))

    def test_write_atomic_warns_when_cleanup_fails_without_hiding_replace_error(self) -> None:
        target = self.root / "target.md"
        replace_error = OSError("replace failed")
        stderr = io.StringIO()

        with mock.patch.object(
            generator, "_replace_with_retry", side_effect=replace_error
        ), mock.patch.object(Path, "unlink", side_effect=PermissionError("cleanup failed")), mock.patch(
            "sys.stderr", stderr
        ):
            with self.assertRaisesRegex(OSError, "replace failed"):
                generator._write_atomic(target, "content")

        warning = stderr.getvalue()
        self.assertIn("Warning: could not clean up temporary file", warning)
        self.assertIn(str(target), warning)
        self.assertIn("cleanup failed", warning)


if __name__ == "__main__":
    unittest.main()
