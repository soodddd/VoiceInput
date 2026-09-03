"""Serialised lifecycle wrapper for the local Qwen3-ASR model."""

from __future__ import annotations

import logging
import threading
import time

import config
import model_manager

logger = logging.getLogger(__name__)


class Qwen3ASREngine:
    """Own one model and prevent load/unload/inference races."""

    def __init__(self) -> None:
        self.model: object | None = None
        self.model_source = ""
        self.last_used_time = time.monotonic()
        self.idle_timeout_sec = 0
        self._operation_lock = threading.RLock()
        self._idle_check_thread: threading.Thread | None = None
        self._idle_stop_event = threading.Event()

    @property
    def current_strategy(self) -> str:
        return config.MODEL_STRATEGY

    @property
    def should_unload_after_use(self) -> bool:
        return config.MODEL_STRATEGY == "memory"

    @property
    def is_loaded(self) -> bool:
        with self._operation_lock:
            return self.model is not None

    def load(self, model_path: str | None = None) -> None:
        """Load an explicitly local model; never trigger an implicit hub download."""
        with self._operation_lock:
            if self.model is not None:
                if model_path is None or self.model_source == model_path:
                    return
                self._unload_locked()

            load_from = model_path or model_manager.get_downloaded_model_path()
            if not load_from:
                raise FileNotFoundError(
                    "No complete local model was found. Download or select a model first."
                )
            if not model_manager.is_valid_model_dir(load_from):
                raise ValueError("The selected local model directory is incomplete")

            import torch
            from qwen_asr import Qwen3ASRModel

            if config.DEVICE.startswith("cuda") and not torch.cuda.is_available():
                raise RuntimeError("CUDA is not available; a supported NVIDIA GPU is required")

            strategy = config.STRATEGY_PARAMS.get(
                config.MODEL_STRATEGY,
                config.STRATEGY_PARAMS["balanced"],
            )
            logger.info(
                "Loading local ASR model on %s with strategy=%s",
                config.DEVICE,
                config.MODEL_STRATEGY,
            )
            started = time.monotonic()
            model = Qwen3ASRModel.from_pretrained(
                load_from,
                dtype=torch.bfloat16,
                device_map=config.DEVICE,
                max_inference_batch_size=strategy["max_inference_batch_size"],
                max_new_tokens=strategy["max_new_tokens"],
            )
            self.model = model
            self.model_source = load_from
            self.idle_timeout_sec = strategy["idle_timeout_sec"]
            self.last_used_time = time.monotonic()
            self._restart_idle_monitor_locked()
            logger.info("ASR model loaded in %.1fs", time.monotonic() - started)

    def _restart_idle_monitor_locked(self) -> None:
        self._idle_stop_event.set()
        previous = self._idle_check_thread
        if previous and previous is not threading.current_thread():
            previous.join(timeout=2)
        self._idle_check_thread = None
        self._idle_stop_event = threading.Event()
        if self.idle_timeout_sec > 0:
            self._idle_check_thread = threading.Thread(
                target=self._idle_check_loop,
                daemon=True,
                name="asr-idle-monitor",
            )
            self._idle_check_thread.start()

    def _idle_check_loop(self) -> None:
        while not self._idle_stop_event.wait(15):
            with self._operation_lock:
                if self.model is None:
                    return
                idle_time = time.monotonic() - self.last_used_time
                if idle_time < self.idle_timeout_sec:
                    continue
                logger.info("ASR model idle timeout reached; unloading")
                self._unload_locked(from_idle_thread=True)
                return

    def unload(self) -> bool:
        with self._operation_lock:
            return self._unload_locked()

    def _unload_locked(self, *, from_idle_thread: bool = False) -> bool:
        self._idle_stop_event.set()
        monitor = self._idle_check_thread
        if monitor and not from_idle_thread and monitor is not threading.current_thread():
            monitor.join(timeout=2)
        self._idle_check_thread = None
        if self.model is None:
            return False

        self.model = None
        self.model_source = ""
        try:
            import torch

            if torch.cuda.is_available():
                torch.cuda.empty_cache()
        except (ImportError, RuntimeError):
            logger.debug("Unable to clear CUDA cache", exc_info=True)
        logger.info("ASR model unloaded")
        return True

    def transcribe(
        self,
        audio_path: str,
        language: str | None = None,
    ) -> tuple[str, str | None]:
        with self._operation_lock:
            if self.model is None:
                raise RuntimeError("Model is not loaded")
            self.last_used_time = time.monotonic()
            results = self.model.transcribe(audio=audio_path, language=language)
            self.last_used_time = time.monotonic()
            if not results:
                return "", None
            first = results[0]
            return (
                getattr(first, "text", "") or "",
                getattr(first, "language", None),
            )


engine_instance = Qwen3ASREngine()
