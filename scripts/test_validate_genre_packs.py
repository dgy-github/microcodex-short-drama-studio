import copy
import unittest

from validate_genre_packs import validate_registry


class GenrePackRegistryTests(unittest.TestCase):
    def test_real_registry_is_closed_and_cross_referenced(self) -> None:
        counts = validate_registry()
        self.assertEqual(counts["packs"], 8)
        self.assertEqual(counts["constraint_profiles"], 2)
        self.assertEqual(counts["agent_profiles"], 16)
        self.assertEqual(counts["retrieval_collections"], 1)
        self.assertEqual(counts["regression_manifests"], 8)
        self.assertEqual(counts["human_writing_profiles"], 1)


if __name__ == "__main__":
    unittest.main()
