import sys
import threading
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from asr_engine import Qwen3ASREngine


class EngineStatusTests(unittest.TestCase):
    def test_status_does_not_wait_for_model_operation(self):
        engine = Qwen3ASREngine()
        for model in (None, object()):
            engine.model = model
            result = []
            finished = threading.Event()

            def read_status():
                result.append(engine.is_loaded)
                finished.set()

            with engine._operation_lock:
                reader = threading.Thread(target=read_status, daemon=True)
                reader.start()
                responsive = finished.wait(1)
            reader.join(2)
            self.assertTrue(responsive, 'Status blocked behind model operation')
            self.assertEqual(result, [model is not None])


if __name__ == '__main__':
    unittest.main()
