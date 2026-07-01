"""
VoiceInput v2 — Python ASR Backend Configuration

All runtime configuration is read from CLI arguments (via init_config)
with environment-variable fallbacks.  Module-level constants that never
change (MODEL_NAME, MAX_NEW_TOKENS, etc.) are defined directly.

Other backend modules access mutable settings via ``config.TOKEN``,
``config.PORT``, etc. — never via ``from config import TOKEN`` so that
values set by ``init_config()`` are always visible.
"""

from __future__ import annotations

import logging
import os
from pathlib import Path

logger = logging.getLogger(__name__)

# ── Fixed constants ────────────────────────────────────────────────

HOST: str = "127.0.0.1"

#: HuggingFace / ModelScope model identifier.
MODEL_NAME: str = "Qwen/Qwen3-ASR-0.6B"

#: Short name used for the local sub-directory (last path segment).
MODEL_SUBDIR: str = MODEL_NAME.split("/")[-1]  # "Qwen3-ASR-0.6B"

#: Maximum tokens the ASR decoder may generate per chunk.
MAX_NEW_TOKENS: int = 1024

#: Audio longer than this (seconds) is split at silence boundaries.
CHUNK_THRESHOLD_SEC: float = 25.0

#: Overlap (seconds) between adjacent chunks to avoid cutting words.
CHUNK_OVERLAP_SEC: float = 1.0

#: RMS threshold (dB) below which a frame is considered silent.
SILENCE_THRESHOLD_DB: int = -40

#: Approximate model size in bytes (used for download progress estimation).
EXPECTED_MODEL_SIZE: int = 1_200_000_000  # ~1.2 GB

# ── Mutable runtime settings (set by init_config) ──────────────────

#: Local security token — required for all sensitive endpoints.
TOKEN: str = os.environ.get("ASR_TOKEN", "")

#: HTTP listen port.
PORT: int = int(os.environ.get("ASR_PORT", "8765"))

#: Parent directory for downloaded models.
MODEL_DIR: Path = Path(
    os.environ.get(
        "ASR_MODEL_DIR",
        str(
            Path(os.environ.get("LOCALAPPDATA", str(Path.home() / "AppData" / "Local")))
            / "VoiceInput"
            / "models"
        ),
    )
)

#: Torch device string, e.g. ``"cuda:0"``.
DEVICE: str = os.environ.get("ASR_DEVICE", "cuda:0")

#: Model inference strategy — "fast", "balanced", or "accurate".
MODEL_STRATEGY: str = os.environ.get("ASR_MODEL_STRATEGY", "balanced")

#: Model strategy parameter mapping.
#: "fast" — larger batch, more tokens, no auto-unload
#: "balanced" — medium batch, medium tokens, 30min idle auto-unload
#: "accurate" — smaller batch, most tokens, 30min idle auto-unload
STRATEGY_PARAMS: dict[str, dict[str, int]] = {
    "fast": {"max_inference_batch_size": 8, "max_new_tokens": 2048, "idle_timeout_sec": 0},
    "balanced": {"max_inference_batch_size": 4, "max_new_tokens": 1024, "idle_timeout_sec": 1800},
    "accurate": {"max_inference_batch_size": 2, "max_new_tokens": 4096, "idle_timeout_sec": 1800},
}

# ── Cached GPU availability ────────────────────────────────────────

_gpu_available: bool | None = None


def is_gpu_available() -> bool:
    """Return ``True`` when a CUDA GPU is available.

    The check is performed once and cached.  If ``torch`` is not
    installed the result is ``False``.
    """
    global _gpu_available
    if _gpu_available is None:
        try:
            import torch

            _gpu_available = torch.cuda.is_available()
        except ImportError:
            _gpu_available = False
        except Exception:
            _gpu_available = False
    return _gpu_available


# ── Initialisation entry-point ─────────────────────────────────────


def init_config(
    token: str,
    port: int | None = None,
    model_dir: str | None = None,
    device: str | None = None,
    model_strategy: str | None = None,
) -> None:
    """Populate runtime settings from parsed CLI arguments.

    Called once from ``__main__`` before the server starts.
    """
    global TOKEN, PORT, MODEL_DIR, DEVICE, MODEL_STRATEGY

    TOKEN = token
    if port is not None:
        PORT = port
    if model_dir is not None:
        MODEL_DIR = Path(model_dir)
    if device is not None:
        DEVICE = device
    if model_strategy is not None:
        MODEL_STRATEGY = model_strategy

    logger.info(
        "Config initialised — port=%s  device=%s  model_dir=%s  model_strategy=%s",
        PORT,
        DEVICE,
        MODEL_DIR,
        MODEL_STRATEGY,
    )


def get_model_path() -> Path:
    """Return the expected local path of the model directory.

    This is ``MODEL_DIR / MODEL_SUBDIR``.  The directory may or may not
    exist yet — use :func:`model_manager.is_model_downloaded` to check.
    """
    return MODEL_DIR / MODEL_SUBDIR
