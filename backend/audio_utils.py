"""
VoiceInput v2 — Audio Pre-processing Utilities

Provides a pipeline that transforms an arbitrary WAV file into the
format expected by Qwen3-ASR:

    WAV (any sample-rate, any channels)
      → mono
      → 16 kHz
      → peak-normalised to -1 dBFS
      → silence-trimmed
      → (optionally) chunked at silence boundaries for long audio

All functions operate on NumPy arrays (``float32``, range ``[-1, 1]``).
"""

from __future__ import annotations

import logging
from math import gcd

import numpy as np
import soundfile as sf

import config

logger = logging.getLogger(__name__)

#: Target sample rate for Qwen3-ASR.
TARGET_SR: int = 16000


# ── Core pipeline ──────────────────────────────────────────────────


def load_and_preprocess(wav_path: str) -> tuple[np.ndarray, int]:
    """Load *wav_path* and return ``(audio, sample_rate)``.

    The returned audio is mono, 16 kHz, ``float32``, peak-normalised
    to -1 dBFS, and has leading/trailing silence removed.
    """
    # Read as float32 (soundfile normalises to [-1, 1] for PCM formats)
    data, sr = sf.read(wav_path, dtype="float32")

    # Convert to mono by averaging channels
    if data.ndim > 1:
        data = data.mean(axis=1)

    # Resample to 16 kHz if needed
    if sr != TARGET_SR:
        data = _resample(data, sr, TARGET_SR)
        sr = TARGET_SR

    # Peak normalisation to -1 dBFS  (10^(-1/20) ≈ 0.891)
    peak = float(np.max(np.abs(data))) if data.size > 0 else 0.0
    if peak > 0:
        data = data * (0.891 / peak)

    # Trim leading / trailing silence
    data = trim_silence(
        data,
        sr,
        threshold_db=config.SILENCE_THRESHOLD_DB,
    )

    # Guard against all-silence audio producing an empty array
    if data.size == 0:
        logger.warning("Audio appears to be entirely silent after preprocessing")
        data = np.zeros(int(sr * 0.1), dtype=np.float32)  # 100 ms of silence

    return data.astype(np.float32), sr


def _resample(data: np.ndarray, orig_sr: int, target_sr: int) -> np.ndarray:
    """Resample *data* from *orig_sr* to *target_sr* using polyphase filtering."""
    from scipy.signal import resample_poly

    g = gcd(target_sr, orig_sr)
    up = target_sr // g
    down = orig_sr // g
    resampled = resample_poly(data, up, down)
    return resampled.astype(np.float32)


# ── Silence detection ──────────────────────────────────────────────


def trim_silence(
    audio: np.ndarray,
    sr: int,
    threshold_db: float = -40,
    frame_ms: int = 30,
) -> np.ndarray:
    """Remove leading and trailing silence from *audio*.

    Silence is detected using per-frame RMS energy.  A 50 ms padding is
    retained on each side to avoid clipping the first/last phoneme.
    """
    frame_len = int(sr * frame_ms / 1000)
    if frame_len <= 0 or len(audio) < frame_len:
        return audio

    threshold = 10 ** (threshold_db / 20)

    n_frames = len(audio) // frame_len
    if n_frames == 0:
        return audio

    # Compute RMS energy for each frame (vectorised)
    frames = audio[: n_frames * frame_len].reshape(n_frames, frame_len)
    rms = np.sqrt(np.mean(frames ** 2, axis=1))

    # Find first non-silent frame
    start = 0
    for i in range(n_frames):
        if rms[i] > threshold:
            start = max(0, i * frame_len - int(sr * 0.05))  # 50 ms padding
            break

    # Find last non-silent frame
    end = len(audio)
    for i in range(n_frames - 1, -1, -1):
        if rms[i] > threshold:
            end = min(len(audio), (i + 1) * frame_len + int(sr * 0.05))
            break

    if end <= start:
        # Entirely silent — return original
        return audio

    return audio[start:end]


def find_split_points(
    audio: np.ndarray,
    sr: int,
    threshold_db: float = -38,
    min_silence_ms: int = 300,
) -> list[int]:
    """Return sample indices suitable for splitting long audio.

    A split point is the midpoint of a silence region that lasts at
    least *min_silence_ms* milliseconds.
    """
    frame_ms = 20
    frame_len = int(sr * frame_ms / 1000)
    if frame_len <= 0 or len(audio) < frame_len:
        return []

    threshold = 10 ** (threshold_db / 20)
    min_silence_frames = max(1, int(min_silence_ms / frame_ms))

    n_frames = len(audio) // frame_len
    if n_frames == 0:
        return []

    frames = audio[: n_frames * frame_len].reshape(n_frames, frame_len)
    rms = np.sqrt(np.mean(frames ** 2, axis=1))

    split_points: list[int] = []
    silence_start: int | None = None

    for i in range(n_frames):
        if rms[i] <= threshold:
            if silence_start is None:
                silence_start = i
        else:
            if silence_start is not None:
                silence_len = i - silence_start
                if silence_len >= min_silence_frames:
                    mid = (silence_start + i) // 2
                    split_points.append(mid * frame_len)
                silence_start = None

    # Handle trailing silence
    if silence_start is not None:
        silence_len = n_frames - silence_start
        if silence_len >= min_silence_frames:
            mid = (silence_start + n_frames) // 2
            split_points.append(mid * frame_len)

    return split_points


def chunk_audio(
    audio: np.ndarray,
    sr: int,
    threshold_sec: float | None = None,
) -> list[np.ndarray]:
    """Split *audio* into chunks no longer than *threshold_sec*.

    Splits are made at silence boundaries when possible.  A
    :data:`config.CHUNK_OVERLAP_SEC`-second overlap is kept between
    adjacent chunks so that words at the boundary are not lost.

    If the audio is shorter than *threshold_sec* a single-element list
    containing the original audio is returned.
    """
    if threshold_sec is None:
        threshold_sec = config.CHUNK_THRESHOLD_SEC

    duration = len(audio) / sr
    if duration <= threshold_sec:
        return [audio]

    overlap_samples = int(config.CHUNK_OVERLAP_SEC * sr)
    split_points = find_split_points(audio, sr)

    if not split_points:
        # No silence found — force-split at regular intervals
        chunk_len = int(threshold_sec * sr)
        chunks: list[np.ndarray] = []
        for start in range(0, len(audio), chunk_len):
            chunks.append(audio[start : start + chunk_len])
        return chunks if chunks else [audio]

    # Build chunks respecting the threshold and adding overlap
    chunks: list[np.ndarray] = []
    last = 0

    for sp in split_points:
        # Only split if the current segment is at least 70 % of threshold
        if (sp - last) / sr >= threshold_sec * 0.7:
            chunks.append(audio[last:sp])
            # Next chunk starts *overlap_samples* before the split point
            last = max(0, sp - overlap_samples)

    # Append remaining audio
    if last < len(audio):
        remaining = audio[last:]
        if len(remaining) / sr > 1.0:
            chunks.append(remaining)
        elif chunks:
            # Merge tiny tail into previous chunk
            chunks[-1] = np.concatenate([chunks[-1], remaining])
        else:
            chunks.append(remaining)

    return chunks if chunks else [audio]
