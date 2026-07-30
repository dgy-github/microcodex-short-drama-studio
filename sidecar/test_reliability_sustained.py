import unittest

from reliability_sustained import run_check


class SustainedReliabilityTests(unittest.IsolatedAsyncioTestCase):
    async def test_repeated_start_and_cancel_retains_one_terminal_per_run(self) -> None:
        result = await run_check(25)
        self.assertEqual(result["accepted"], 25)
        self.assertEqual(result["cancelled"], 25)


if __name__ == "__main__":
    unittest.main()
