"""Tests for the endpoint egress guard (SSRF mitigation)."""

import unittest
from unittest import mock

from endpoint_guard import assert_public_https_endpoint


class EndpointGuardTests(unittest.TestCase):
    @mock.patch(
        "endpoint_guard._resolved_addresses",
        return_value=("93.184.216.34",),
    )
    def test_public_https_passes(self, _resolved_addresses: mock.Mock) -> None:
        assert_public_https_endpoint(
            "https://api.teamorouter.cn/v1/chat/completions"
        )

    def test_plain_http_is_refused(self) -> None:
        with self.assertRaises(ValueError):
            assert_public_https_endpoint("http://api.teamorouter.cn/v1")

    def test_loopback_names_and_addresses_are_refused(self) -> None:
        for url in ("https://localhost/v1", "https://127.0.0.1/v1", "https://[::1]/v1"):
            with self.subTest(url=url):
                with self.assertRaises(ValueError):
                    assert_public_https_endpoint(url)

    def test_private_and_link_local_ranges_are_refused(self) -> None:
        for url in (
            "https://10.0.0.5/v1",
            "https://192.168.1.4/v1",
            "https://172.16.0.9/v1",
            "https://169.254.169.254/latest/meta-data",
        ):
            with self.subTest(url=url):
                with self.assertRaises(ValueError):
                    assert_public_https_endpoint(url)

    def test_url_without_host_is_refused(self) -> None:
        with self.assertRaises(ValueError):
            assert_public_https_endpoint("https:///v1")


if __name__ == "__main__":
    unittest.main()
