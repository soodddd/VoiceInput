"""Thread-safe local model discovery and download management."""

from __future__ import annotations

import json
import logging
import os
import shutil
import threading
import time
from pathlib import Path

import config

logger = logging.getLogger(__name__)

_EXPECTED_SIZE: int = config.EXPECTED_MODEL_SIZE
_MODEL_MARKER = "selected_model.json"
_VALID_SOURCES = {"modelscope", "huggingface", "local"}

_state_lock = threading.RLock()
_download_status: dict[str, object] = {
    "state": "idle",
    "downloading": False,
    "progress": 0.0,
    "speed": 0.0,
    "error": None,
    "message": "Ready",
}
_downloaded_model_path: str | None = None
_cancel_requested = False
_download_generation = 0


def _snapshot_status() -> dict[str, object]:
    with _state_lock:
        return dict(_download_status)


def _set_status(**updates: object) -> None:
    with _state_lock:
        _download_status.update(updates)


def _marker_path(model_dir: str | os.PathLike | None = None) -> Path:
    return Path(model_dir or config.MODEL_DIR) / _MODEL_MARKER


def _write_marker(path: str, model_dir: str | os.PathLike | None = None) -> None:
    marker = _marker_path(model_dir)
    marker.parent.mkdir(parents=True, exist_ok=True)
    temporary = marker.with_suffix(".tmp")
    temporary.write_text(
        json.dumps({"path": str(Path(path).resolve())}, ensure_ascii=False),
        encoding="utf-8",
    )
    os.replace(temporary, marker)


def _read_marker(model_dir: str | os.PathLike | None = None) -> str | None:
    try:
        value = json.loads(_marker_path(model_dir).read_text(encoding="utf-8"))
        path = value.get("path")
        return str(path) if path else None
    except (OSError, ValueError, TypeError, AttributeError):
        return None


def _candidate_model_dirs(model_dir: str | os.PathLike | None = None) -> list[Path]:
    base = Path(model_dir or config.MODEL_DIR)
    org, _, name = config.MODEL_NAME.partition("/")
    candidates = [
        base / config.MODEL_SUBDIR,
        base / config.MODEL_NAME.replace("/", "_"),
        base / org / name if name else base / org,
        base / config.MODEL_NAME.replace("/", os.sep),
    ]
    # Backward compatibility for earlier HuggingFace cache downloads.
    if name:
        candidates.append(
            base / f"models--{org}--{name}" / "snapshots"
        )
    return candidates


def is_model_downloaded(model_dir: str | os.PathLike | None = None) -> bool:
    """Discover a complete local model and remember its concrete path."""
    global _downloaded_model_path

    with _state_lock:
        known = _downloaded_model_path
    if known and _is_valid_model_dir(known):
        return True

    marked = _read_marker(model_dir)
    if marked and _is_valid_model_dir(marked):
        with _state_lock:
            _downloaded_model_path = marked
        return True

    for candidate in _candidate_model_dirs(model_dir):
        if candidate.name == "snapshots" and candidate.is_dir():
            try:
                snapshot_dirs = sorted(
                    (item for item in candidate.iterdir() if item.is_dir()),
                    key=lambda item: item.stat().st_mtime,
                    reverse=True,
                )
            except OSError:
                snapshot_dirs = []
            for snapshot in snapshot_dirs:
                if _is_valid_model_dir(snapshot):
                    with _state_lock:
                        _downloaded_model_path = str(snapshot.resolve())
                    _write_marker(_downloaded_model_path, model_dir)
                    return True
        elif _is_valid_model_dir(candidate):
            with _state_lock:
                _downloaded_model_path = str(candidate.resolve())
            _write_marker(_downloaded_model_path, model_dir)
            return True
    return False


def get_downloaded_model_path() -> str | None:
    with _state_lock:
        known = _downloaded_model_path
    if known and _is_valid_model_dir(known):
        return known
    return _downloaded_model_path if is_model_downloaded() else None


def get_download_status() -> dict[str, object]:
    return _snapshot_status()


def is_valid_model_dir(path: str | os.PathLike) -> bool:
    return _is_valid_model_dir(path)


def cancel_download() -> bool:
    """Request cancellation without advertising completion prematurely."""
    global _cancel_requested
    with _state_lock:
        if not bool(_download_status["downloading"]):
            return False
        _cancel_requested = True
        _download_status.update(
            state="cancelling",
            message="Cancelling download…",
            error=None,
        )
    logger.info("Model download cancellation requested")
    return True


def _is_cancel_requested(generation: int | None = None) -> bool:
    with _state_lock:
        if generation is not None and generation != _download_generation:
            return True
        return _cancel_requested


def start_download(
    source: str,
    model_dir: str | os.PathLike | None = None,
    local_path: str | None = None,
) -> bool:
    """Validate input and start exactly one background operation."""
    global _cancel_requested, _download_generation

    source = source.strip().lower()
    if source not in _VALID_SOURCES:
        raise ValueError(f"Unsupported model source: {source}")
    if source == "local":
        if not local_path:
            raise ValueError("A local model directory is required")
        if not _is_valid_model_dir(local_path):
            raise ValueError("The selected directory is not a complete model")

    target_root = Path(model_dir or config.MODEL_DIR)
    target_root.mkdir(parents=True, exist_ok=True)
    if source != "local":
        free_bytes = shutil.disk_usage(target_root).free
        required = max(_EXPECTED_SIZE, 1_500_000_000)
        if free_bytes < required:
            raise OSError(
                f"Not enough disk space: {free_bytes / 1024**3:.1f} GiB free, "
                f"at least {required / 1024**3:.1f} GiB required"
            )

    with _state_lock:
        if bool(_download_status["downloading"]):
            return False
        _download_generation += 1
        generation = _download_generation
        _cancel_requested = False
        _download_status.update(
            state="downloading",
            downloading=True,
            progress=0.0,
            speed=0.0,
            error=None,
            message="Validating local model…" if source == "local" else "Downloading model…",
        )

    threading.Thread(
        target=_download_worker,
        args=(generation, source, str(target_root), local_path),
        daemon=True,
        name=f"model-download-{generation}",
    ).start()
    return True


def download_model(
    source: str,
    model_dir: str | os.PathLike,
    local_path: str | None = None,
) -> str:
    """Synchronously download to a deterministic directory or validate local data."""
    target = Path(model_dir) / config.MODEL_SUBDIR
    target.parent.mkdir(parents=True, exist_ok=True)

    if source == "modelscope":
        from modelscope import snapshot_download

        path = snapshot_download(
            model_id=config.MODEL_NAME,
            local_dir=str(target),
        )
    elif source == "huggingface":
        from huggingface_hub import snapshot_download

        path = snapshot_download(
            repo_id=config.MODEL_NAME,
            local_dir=str(target),
        )
    elif source == "local":
        if not local_path or not _is_valid_model_dir(local_path):
            raise ValueError("The selected directory is not a complete model")
        path = str(Path(local_path).resolve())
    else:
        raise ValueError(f"Unsupported model source: {source}")

    if not _is_valid_model_dir(path):
        raise RuntimeError("Download finished but required model files are missing")
    return str(Path(path).resolve())


def _download_worker(
    generation: int,
    source: str,
    model_dir: str,
    local_path: str | None,
) -> None:
    global _downloaded_model_path

    monitor_stop = threading.Event()
    monitor: threading.Thread | None = None
    if source != "local":
        monitor = threading.Thread(
            target=_monitor_progress,
            args=(generation, model_dir, monitor_stop),
            daemon=True,
            name=f"model-progress-{generation}",
        )
        monitor.start()

    try:
        if _is_cancel_requested(generation):
            raise InterruptedError("Download cancelled")
        path = download_model(source, model_dir, local_path)
        if _is_cancel_requested(generation):
            raise InterruptedError("Download cancelled")
        _write_marker(path, model_dir)
        with _state_lock:
            if generation == _download_generation:
                _downloaded_model_path = path
                _download_status.update(
                    state="completed",
                    downloading=False,
                    progress=100.0,
                    speed=0.0,
                    error=None,
                    message="Model is ready",
                )
        logger.info("Model is ready at %s", path)
    except InterruptedError:
        with _state_lock:
            if generation == _download_generation:
                _download_status.update(
                    state="cancelled",
                    downloading=False,
                    speed=0.0,
                    error=None,
                    message="Download cancelled",
                )
    except Exception as exc:
        with _state_lock:
            if generation == _download_generation:
                _download_status.update(
                    state="error",
                    downloading=False,
                    speed=0.0,
                    error=str(exc),
                    message=f"Download failed: {exc}",
                )
        logger.error("Model download failed: %s", exc, exc_info=True)
    finally:
        monitor_stop.set()
        if monitor and monitor is not threading.current_thread():
            monitor.join(timeout=1.5)
        with _state_lock:
            if generation == _download_generation:
                _download_status["downloading"] = False


def _monitor_progress(
    generation: int,
    target_dir: str,
    stop_event: threading.Event,
) -> None:
    last_size = _get_dir_size(target_dir)
    last_time = time.monotonic()
    while not stop_event.wait(0.75):
        if _is_cancel_requested(generation):
            return
        try:
            current_size = _get_dir_size(target_dir)
            now = time.monotonic()
            elapsed = max(now - last_time, 0.001)
            progress = (
                min(99.0, current_size / _EXPECTED_SIZE * 100.0)
                if _EXPECTED_SIZE > 0
                else 0.0
            )
            _set_status(
                progress=progress,
                speed=max(0.0, (current_size - last_size) / elapsed),
            )
            last_size, last_time = current_size, now
        except OSError:
            logger.debug("Unable to inspect download directory", exc_info=True)


def _get_dir_size(path: str | os.PathLike) -> int:
    total = 0
    try:
        for entry in Path(path).rglob("*"):
            try:
                if entry.is_file() and not entry.is_symlink():
                    total += entry.stat().st_size
            except OSError:
                continue
    except OSError:
        pass
    return total


def _is_valid_model_dir(path: str | os.PathLike) -> bool:
    """Require both metadata and at least one plausible model weight file."""
    try:
        candidate = Path(path)
        if not candidate.is_dir() or not (candidate / "config.json").is_file():
            return False
        patterns = ("*.safetensors", "*.bin", "*.pt", "*.pth")
        return any(next(candidate.glob(pattern), None) is not None for pattern in patterns)
    except (OSError, ValueError):
        return False
