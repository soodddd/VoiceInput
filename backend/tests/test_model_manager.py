import json
import sys
import tempfile
import threading
import time
import unittest
from pathlib import Path
from unittest import mock

BACKEND = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(BACKEND))

import config
import model_manager


class ModelManagerTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.model_root = Path(self.directory.name)
        self.config_patch = mock.patch.object(config, "MODEL_DIR", self.model_root)
        self.config_patch.start()
        self.addCleanup(self.config_patch.stop)
        with model_manager._state_lock:
            model_manager._downloaded_model_path = None
            model_manager._download_status.update(
                state="idle",
                downloading=False,
                progress=0.0,
                speed=0.0,
                error=None,
                message="Ready",
            )

    def test_incomplete_directory_is_not_a_model(self):
        candidate = self.model_root / config.MODEL_SUBDIR
        candidate.mkdir()
        (candidate / "config.json").write_text("{}", encoding="utf-8")
        self.assertFalse(model_manager.is_model_downloaded())

    def test_complete_local_model_is_discovered(self):
        candidate = self.model_root / config.MODEL_SUBDIR
        candidate.mkdir()
        (candidate / "config.json").write_text(json.dumps({}), encoding="utf-8")
        (candidate / "model.safetensors").write_bytes(b"weights")
        self.assertTrue(model_manager.is_model_downloaded())
        self.assertEqual(
            Path(model_manager.get_downloaded_model_path()).resolve(),
            candidate.resolve(),
        )

    def test_cancel_does_not_allow_a_second_worker_until_first_exits(self):
        entered = threading.Event()
        release = threading.Event()

        def blocked_download(_source, model_dir, _local_path=None):
            entered.set()
            release.wait(2)
            candidate = Path(model_dir) / config.MODEL_SUBDIR
            candidate.mkdir(exist_ok=True)
            (candidate / "config.json").write_text("{}", encoding="utf-8")
            (candidate / "model.safetensors").write_bytes(b"weights")
            return str(candidate)

        with mock.patch.object(model_manager, "download_model", blocked_download), mock.patch.object(
            model_manager.shutil, "disk_usage"
        ) as disk_usage:
            disk_usage.return_value = mock.Mock(free=10_000_000_000)
            self.assertTrue(model_manager.start_download("huggingface"))
            self.assertTrue(entered.wait(1))
            self.assertTrue(model_manager.cancel_download())
            self.assertTrue(model_manager.get_download_status()["downloading"])
            self.assertFalse(model_manager.start_download("huggingface"))
            release.set()
            deadline = time.monotonic() + 2
            while model_manager.get_download_status()["downloading"] and time.monotonic() < deadline:
                time.sleep(0.01)
            self.assertEqual(model_manager.get_download_status()["state"], "cancelled")


if __name__ == "__main__":
    unittest.main()
