"""
VoiceInput v2 — Model Download & Path Management

Supports downloading the Qwen3-ASR-0.6B model from three sources:

* **ModelScope** — recommended for users in mainland China.
* **HuggingFace** — international users.
* **local**      — user supplies an existing model directory.

Downloads run in a background thread.  Progress is tracked via a global
``_download_status`` dictionary that the FastAPI ``/model/download/status``
endpoint polls (per architecture decision Q-A2: polling, not SSE).
"""

from __future__ import annotations

import logging
import os
import threading
import time
from pathlib import Path

import config

logger = logging.getLogger(__name__)

# ── Global state ───────────────────────────────────────────────────

#: Approximate model size in bytes — used for progress estimation.
_EXPECTED_SIZE: int = config.EXPECTED_MODEL_SIZE

#: Current download status, polled by ``/model/download/status``.
_download_status: dict[str, object] = {
    "downloading": False,
    "progress": 0.0,
    "speed": 0.0,
    "error": None,
}

#: Path to the downloaded model (set after successful download or local
#: path validation).  ``None`` means the model has not been located yet.
_downloaded_model_path: str | None = None

#: Serialises download-start checks.
_download_lock = threading.Lock()

#: Cancellation flag — set by :func:`cancel_download`, polled by the
#: download worker thread.  Reset to ``False`` at the start of each new
#: download.
_cancel_requested: bool = False

#: Lock protecting :data:`_cancel_requested`.
_cancel_lock = threading.Lock()


# ── Public API ─────────────────────────────────────────────────────


def is_model_downloaded(model_dir: str | os.PathLike | None = None) -> bool:
    """Return ``True`` when the model appears to be present on disk.

    Checks several candidate locations:

    1. ``_downloaded_model_path`` (set after a successful download).
    2. ``model_dir / MODEL_SUBDIR`` (direct sub-directory).
    3. ``model_dir / <org> / MODEL_SUBDIR`` (HF / ModelScope cache layout).
    4. ``model_dir / MODEL_NAME`` with ``/`` replaced by ``_``.

    A directory is considered a valid model if it contains
    ``config.json``.
    """
    global _downloaded_model_path

    # 1. Already known path
    if _downloaded_model_path and _is_valid_model_dir(_downloaded_model_path):
        return True

    if model_dir is None:
        model_dir = config.MODEL_DIR
    model_dir = Path(model_dir)

    model_subdir = config.MODEL_SUBDIR
    org_name = config.MODEL_NAME.split("/")[0] if "/" in config.MODEL_NAME else ""

    candidates: list[Path] = [
        model_dir / model_subdir,
        model_dir / org_name / model_subdir if org_name else model_dir / model_subdir,
        model_dir / config.MODEL_NAME.replace("/", "_"),
        model_dir / config.MODEL_NAME.replace("/", os.sep),
    ]

    for candidate in candidates:
        if _is_valid_model_dir(candidate):
            _downloaded_model_path = str(candidate)
            logger.info("Model found at %s", _downloaded_model_path)
            return True

    return False


def get_downloaded_model_path() -> str | None:
    """Return the local filesystem path of the model, or ``None``."""
    global _downloaded_model_path

    if _downloaded_model_path and _is_valid_model_dir(_downloaded_model_path):
        return _downloaded_model_path

    # Try to discover the path
    if is_model_downloaded():
        return _downloaded_model_path

    return None


def get_download_status() -> dict[str, object]:
    """Return a snapshot of the current download status."""
    return dict(_download_status)


def cancel_download() -> bool:
    """Request cancellation of an in-progress download.

    Sets a flag that the download worker checks between operations.  The
    actual SDK download (modelscope / huggingface_hub) may take a moment
    to terminate, but the UI can immediately reflect the cancelled state.

    Returns ``True`` if a cancellation was requested (i.e. a download was
    in progress), ``False`` if no download was running.
    """
    global _cancel_requested

    with _cancel_lock:
        if not _download_status["downloading"]:
            return False
        _cancel_requested = True
        logger.info("Download cancellation requested")

    # Reflect the cancellation immediately in the public status so the UI
    # can stop its progress polling without waiting for the worker thread
    # to wind down.
    _download_status["downloading"] = False
    _download_status["error"] = "Download cancelled by user"
    return True


def _is_cancel_requested() -> bool:
    """Thread-safe check of the cancellation flag."""
    with _cancel_lock:
        return _cancel_requested


def _reset_cancel_flag() -> None:
    """Reset the cancellation flag at the start of a new download."""
    global _cancel_requested
    with _cancel_lock:
        _cancel_requested = False


def start_download(
    source: str,
    model_dir: str | os.PathLike | None = None,
    local_path: str | None = None,
) -> bool:
    """Start a background download.

    Returns ``True`` if the download was started, ``False`` if another
    download is already in progress.
    """
    with _download_lock:
        if _download_status["downloading"]:
            return False

        _reset_cancel_flag()
        _download_status["downloading"] = True
        _download_status["progress"] = 0.0
        _download_status["speed"] = 0.0
        _download_status["error"] = None

    if model_dir is None:
        model_dir = config.MODEL_DIR

    thread = threading.Thread(
        target=_download_worker,
        args=(source, str(model_dir), local_path),
        daemon=True,
    )
    thread.start()
    return True


def download_model(
    source: str,
    model_dir: str | os.PathLike,
    local_path: str | None = None,
) -> str:
    """Synchronously download (or validate) the model.

    This is the blocking implementation called by :func:`start_download`
    in a background thread.  It can also be called directly from scripts.

    Returns the local path to the model directory.
    """
    global _downloaded_model_path

    model_dir = str(model_dir)

    if source == "modelscope":
        logger.info("Downloading %s from ModelScope to %s", config.MODEL_NAME, model_dir)
        from modelscope import snapshot_download

        path = snapshot_download(
            model_id=config.MODEL_NAME,
            cache_dir=model_dir,
        )
        _downloaded_model_path = str(path)

    elif source == "huggingface":
        logger.info(
            "Downloading %s from HuggingFace to %s", config.MODEL_NAME, model_dir
        )
        from huggingface_hub import snapshot_download as hf_snapshot_download

        path = hf_snapshot_download(
            repo_id=config.MODEL_NAME,
            cache_dir=model_dir,
        )
        _downloaded_model_path = str(path)

    elif source == "local":
        if not local_path or not os.path.isdir(local_path):
            raise ValueError(
                "source='local' requires a valid local_path directory"
            )
        if not _is_valid_model_dir(local_path):
            raise ValueError(
                f"local_path '{local_path}' does not contain a valid model "
                f"(missing config.json)"
            )
        _downloaded_model_path = local_path
        logger.info("Using local model at %s", local_path)

    else:
        raise ValueError(
            f"Unknown source '{source}'. "
            f"Expected 'modelscope', 'huggingface', or 'local'."
        )

    return _downloaded_model_path


# ── Internal helpers ───────────────────────────────────────────────


def _download_worker(
    source: str,
    model_dir: str,
    local_path: str | None,
) -> None:
    """Background thread: run download with progress monitoring."""
    global _downloaded_model_path

    # For hub downloads, monitor directory growth in a separate thread
    monitor_stop = threading.Event()
    if source in ("modelscope", "huggingface"):
        monitor_thread = threading.Thread(
            target=_monitor_progress,
            args=(model_dir, monitor_stop),
            daemon=True,
        )
        monitor_thread.start()

    try:
        # Check cancellation before kicking off the (potentially heavy) SDK call
        if _is_cancel_requested():
            logger.info("Download cancelled before start")
            return

        path = download_model(source, model_dir, local_path)

        # Check cancellation again after the SDK returns (the SDK may have
        # completed despite a cancel request — in that case we still honor
        # the user's intent and discard the result).
        if _is_cancel_requested():
            logger.info("Download completed but cancellation was requested — discarding")
            _downloaded_model_path = None
            return

        _downloaded_model_path = path
        _download_status["progress"] = 100.0
        _download_status["speed"] = 0.0
        _download_status["error"] = None
        logger.info("Model download complete: %s", path)
    except Exception as exc:
        if _is_cancel_requested():
            _download_status["error"] = "Download cancelled by user"
            logger.info("Model download cancelled")
        else:
            _download_status["error"] = str(exc)
            logger.error("Model download failed: %s", exc, exc_info=True)
    finally:
        monitor_stop.set()
        _download_status["downloading"] = False


def _monitor_progress(target_dir: str, stop_event: threading.Event) -> None:
    """Periodically estimate download progress from directory size."""
    last_size = 0
    last_time = time.time()

    while not stop_event.wait(0.5):
        # Stop monitoring if the download was cancelled
        if _is_cancel_requested():
            return

        try:
            current_size = _get_dir_size(target_dir)
            now = time.time()
            dt = now - last_time

            if dt > 0:
                speed = (current_size - last_size) / dt
                _download_status["speed"] = max(0.0, speed)

            last_size = current_size
            last_time = now

            if _EXPECTED_SIZE > 0:
                progress = min(99.0, (current_size / _EXPECTED_SIZE) * 100.0)
                _download_status["progress"] = progress
        except Exception:
            pass


def _get_dir_size(path: str) -> int:
    """Return total size in bytes of all files under *path*."""
    total = 0
    if not os.path.isdir(path):
        return total
    for dirpath, _dirnames, filenames in os.walk(path):
        for f in filenames:
            fp = os.path.join(dirpath, f)
            try:
                if not os.path.islink(fp):
                    total += os.path.getsize(fp)
            except OSError:
                pass
    return total


def _is_valid_model_dir(path: str | os.PathLike) -> bool:
    """Return ``True`` if *path* is a directory containing ``config.json``."""
    try:
        p = Path(path)
        return p.is_dir() and (p / "config.json").is_file()
    except (OSError, ValueError):
        return False
