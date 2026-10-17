import http.client
import json
from pathlib import Path
import tempfile
import threading
import unittest

import trace as capture


class CollectorTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.directory = Path(self.tmp.name)
        self.manifest = {"build": "test-only", "token": "a" * 64}
        self.collector = capture.Collector(self.directory, self.manifest)

    def tearDown(self):
        self.tmp.cleanup()

    def batch(self, sequence=1):
        return {"schema": capture.SCHEMA, "build": "test-only", "page": "synthetic-test",
                "dropped": 0, "observerErrors": 0,
                "records": [{"sequence": sequence, "time": 1, "channel": "event", "phase": "receive",
                             "name": 246368015, "payload": {"regionBoxID": 42}, "requestSequence": None}]}

    def test_retry_and_restart_deduplication(self):
        body = self.batch()
        self.collector.accept(body)
        self.collector.accept(body)
        restarted = capture.Collector(self.directory, self.manifest)
        restarted.accept(body)
        restarted.accept(self.batch(3))
        lines = self.collector.path.read_text(encoding="utf-8").splitlines()
        self.assertEqual(len(lines), 2)
        self.assertEqual(restarted.summary["records"], 2)
        self.assertEqual(restarted.summary["pages"]["synthetic-test"]["lastSequence"], 3)

    def test_reject_before_write_and_size_limit(self):
        bad = self.batch()
        bad["build"] = "wrong-build"
        with self.assertRaises(ValueError):
            self.collector.accept(bad)
        bad = self.batch()
        bad["records"] += [dict(bad["records"][0])]
        with self.assertRaises(ValueError):
            self.collector.accept(bad)
        self.collector.limit = 1
        with self.assertRaises(OverflowError):
            self.collector.accept(self.batch())
        self.assertFalse(self.collector.path.exists())

    def test_http_capability_cors_and_persistence(self):
        server = capture.server_for(self.collector, 0)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        connection = http.client.HTTPConnection("127.0.0.1", server.server_port, timeout=3)
        try:
            route = "/capture/" + self.manifest["token"]
            connection.request("OPTIONS", route, headers={"Origin": "null"})
            response = connection.getresponse()
            self.assertEqual(response.status, 204)
            self.assertEqual(response.getheader("Access-Control-Allow-Origin"), "*")
            response.read()
            connection.request("POST", "/capture/wrong", json.dumps(self.batch()))
            self.assertEqual(connection.getresponse().status, 404)
            connection.close()
            connection.request("POST", route, json.dumps(self.batch()), {"Content-Type": "text/plain;charset=UTF-8"})
            response = connection.getresponse()
            self.assertEqual(response.status, 204)
            response.read()
            self.assertEqual(json.loads((self.directory / "status.json").read_text())["records"], 1)
        finally:
            connection.close()
            server.shutdown()
            server.server_close()
            thread.join()

    def test_localizations_have_matching_keys(self):
        self.assertEqual(set(capture.TEXT["zh-CN"]), set(capture.TEXT["en-US"]))

    def test_known_failed_and_legacy_builds_cannot_reinstall(self):
        for stage in (None, "observe"):
            manifest = dict(self.manifest)
            if stage:
                manifest["stage"] = stage
            capture.save(self.directory / "manifest.json", manifest)
            with self.assertRaisesRegex(ValueError, "known_black_screen_regression"):
                capture.install(self.directory)

    def test_report_joins_query_results_and_keeps_unknown_counts(self):
        def encode(value):
            if isinstance(value, list):
                return {"$array": [{"key": str(i), "value": encode(v)} for i, v in enumerate(value)], "length": len(value)}
            if isinstance(value, dict):
                return {k: encode(v) for k, v in value.items()}
            return value
        body = self.batch()
        body["records"][0].update(channel="query", phase="result", name=encode(["RegionTemplates"]),
            payload=encode([[{"regionTemplates": [{"name": "CapeTrinity", "key": 1,
                "map": {"cities": [{"uid": 1026}, {"uid": 1027}]}, "boxes": {"$cut": "budget"}}]}]]))
        self.collector.accept(body)
        result = capture.report(self.directory)
        finding = result["templateResponses"][0]
        self.assertTrue(finding["truncated"])
        self.assertEqual(finding["templates"][0]["cityCount"], 2)
        self.assertEqual(finding["templates"][0]["cityUIDs"], [1026, 1027])
        self.assertIsNone(finding["templates"][0]["boxCount"])


if __name__ == "__main__":
    unittest.main()
