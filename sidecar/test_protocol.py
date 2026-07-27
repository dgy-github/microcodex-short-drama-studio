import unittest

from campaign_adapter.protocol import PROTOCOL, make_event


class ProtocolTests(unittest.TestCase):
    def test_event_uses_shared_protocol(self) -> None:
        event = make_event(
            seq=1,
            job_id="job_1",
            run_id="run_1",
            event_type="run.started",
            payload={},
        )
        self.assertEqual(event.protocol, PROTOCOL)
        self.assertEqual(event.seq, 1)

    def test_zero_sequence_is_rejected(self) -> None:
        with self.assertRaises(ValueError):
            make_event(
                seq=0,
                job_id="job_1",
                run_id="run_1",
                event_type="run.started",
                payload={},
            )


if __name__ == "__main__":
    unittest.main()

