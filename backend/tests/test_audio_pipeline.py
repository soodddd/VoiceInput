import sys
import types
import unittest
from pathlib import Path
from unittest import mock

import numpy as np

try:
    import soundfile  # noqa: F401
except ImportError:
    sys.modules["soundfile"] = types.ModuleType("soundfile")

BACKEND = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(BACKEND))

from audio_utils import chunk_audio, load_and_preprocess, trim_silence
from postprocess import merge_transcription_chunks


class AudioPipelineTests(unittest.TestCase):
    def test_all_silence_is_rejected(self):
        with mock.patch(
            "audio_utils.sf.read",
            return_value=(np.zeros(16_000, dtype=np.float32), 16_000),
            create=True,
        ):
            with self.assertRaisesRegex(ValueError, "No speech"):
                load_and_preprocess("silence.wav")

    def test_trim_happens_before_normalisation(self):
        sample_rate = 16_000
        audio = np.concatenate(
            [
                np.zeros(sample_rate // 2, dtype=np.float32),
                np.full(sample_rate, 0.05, dtype=np.float32),
                np.zeros(sample_rate // 2, dtype=np.float32),
            ]
        )
        trimmed = trim_silence(audio, sample_rate, threshold_db=-40)
        self.assertGreater(trimmed.size, sample_rate)
        self.assertLess(trimmed.size, audio.size)

    def test_chunks_have_a_hard_upper_bound_plus_overlap(self):
        sample_rate = 1_000
        audio = np.ones(65 * sample_rate, dtype=np.float32)
        chunks = chunk_audio(audio, sample_rate, threshold_sec=25)
        self.assertEqual(len(chunks), 3)
        self.assertTrue(all(len(chunk) <= 26 * sample_rate for chunk in chunks))

    def test_overlap_text_is_not_duplicated(self):
        merged = merge_transcription_chunks(["今天测试语音输入", "语音输入非常流畅"])
        self.assertEqual(merged, "今天测试语音输入非常流畅")


if __name__ == "__main__":
    unittest.main()
