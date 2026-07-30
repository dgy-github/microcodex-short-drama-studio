import tempfile
import unittest
from pathlib import Path

from verify_pinned_python_dependency import pinned_requirement, verify_direct_url


REVISION = "1d935714449d18cad5bdc6711a498297ed73a5fb"
URL = "https://github.com/dgy-github/campaign-muti-agent.git"


class PinnedPythonDependencyTests(unittest.TestCase):
    def test_extracts_exact_https_git_requirement(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            project = Path(temporary) / "pyproject.toml"
            project.write_text(
                "[project]\n"
                "dependencies = [\n"
                f'  "campaign-muti-agent @ git+{URL}@{REVISION}"\n'
                "]\n",
                encoding="utf-8",
            )
            requirement, url, revision = pinned_requirement(project)
        self.assertIn(REVISION, requirement)
        self.assertEqual(url, URL)
        self.assertEqual(revision, REVISION)

    def test_rejects_a_branch_instead_of_an_exact_revision(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            project = Path(temporary) / "pyproject.toml"
            project.write_text(
                "[project]\n"
                f'dependencies = ["campaign-muti-agent @ git+{URL}@main"]\n',
                encoding="utf-8",
            )
            with self.assertRaisesRegex(ValueError, "exact HTTPS git revision"):
                pinned_requirement(project)

    def test_direct_url_must_match_source_and_both_revision_fields(self) -> None:
        direct_url = {
            "url": URL,
            "vcs_info": {
                "vcs": "git",
                "commit_id": REVISION,
                "requested_revision": REVISION,
            },
        }
        verify_direct_url(direct_url, URL, REVISION)
        direct_url["vcs_info"]["commit_id"] = "a" * 40
        with self.assertRaisesRegex(ValueError, "commit does not match"):
            verify_direct_url(direct_url, URL, REVISION)


if __name__ == "__main__":
    unittest.main()
