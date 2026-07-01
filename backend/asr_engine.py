"""
VoiceInput v2 — Qwen3-ASR Engine Wrapper

Encapsulates model loading, inference, and unloading behind a simple
class interface.  A module-level singleton ``engine_instance`` is used
by the FastAPI server.

Heavy imports (``torch``, ``qwen_asr``) are deferred to :meth:`load`
so that the server can start and respond to ``/health`` even before
PyTorch is fully initialised.
"""

from __future__ import annotations

import logging
import threading
import time

import config
import model_manager

logger = logging.getLogger(__name__)


class Qwen3ASREngine:
    """Thin wrapper around ``qwen_asr.Qwen3ASRModel``.

    Attributes
    ----------
    model : object | None
        The underlying ``Qwen3ASRModel`` instance, or ``None`` when
        not loaded.
    model_source : str
        Human-readable description of what was loaded (local path or
        hub ID).
    """

    def __init__(self) -> None:
        self.model: object | None = None
        self.model_source: str = ""
        self.last_used_time: float = time.time()
        self.idle_timeout_sec: int = 0  # Set during load() based on strategy
        self._idle_check_thread: threading.Thread | None = None
        self._idle_stop_event = threading.Event()

    @property
    def current_strategy(self) -> str:
        """Return the active model strategy name."""
        return config.MODEL_STRATEGY

    @property
    def should_unload_after_use(self) -> bool:
        """Return True when the 'memory' strategy is active.

        In this mode the model is released from GPU memory immediately
        after each full transcription request completes, minimising
        VRAM usage at the cost of re-loading latency on the next use.
        """
        return config.MODEL_STRATEGY == "memory"

    # ── Lifecycle ──────────────────────────────────────────────────

    @property
    def is_loaded(self) -> bool:
        """Return ``True`` when a model is currently in memory."""
        return self.model is not None

    def load(self, model_name: str | None = None) -> None:
        """Load the ASR model onto the GPU.

        Parameters
        ----------
        model_name : str | None
            Override the model identifier / path.  When ``None`` the
            engine tries the locally downloaded model first, falling
            back to :data:`config.MODEL_NAME` (which triggers a hub
            download).
        """
        if self.model is not None:
            self.unload()

        import torch
        from qwen_asr import Qwen3ASRModel

        # Determine what to load
        if model_name:
            load_from: str = model_name
        else:
            local_path = model_manager.get_downloaded_model_path()
            if local_path:
                load_from = local_path
            else:
                load_from = config.MODEL_NAME

        logger.info(
            "Loading ASR model from '%s' on %s (dtype=bfloat16, max_new_tokens=%d)",
            load_from,
            config.DEVICE,
            config.MAX_NEW_TOKENS,
        )

        strategy = config.STRATEGY_PARAMS.get(config.MODEL_STRATEGY, config.STRATEGY_PARAMS["balanced"])
        batch_size = strategy["max_inference_batch_size"]
        max_tokens = strategy["max_new_tokens"]

        t0 = time.time()
        self.model = Qwen3ASRModel.from_pretrained(
            load_from,
            dtype=torch.bfloat16,
            device_map=config.DEVICE,
            max_inference_batch_size=batch_size,
            max_new_tokens=max_tokens,
        )
        self.model_source = load_from
        elapsed = time.time() - t0
        logger.info("Model loaded in %.1fs", elapsed)

        self.idle_timeout_sec = strategy["idle_timeout_sec"]
        self.last_used_time = time.time()

        # Start idle check thread if auto-unload is enabled
        if self.idle_timeout_sec > 0:
            self._idle_stop_event.clear()
            self._idle_check_thread = threading.Thread(
                target=self._idle_check_loop,
                daemon=True,
            )
            self._idle_check_thread.start()

    def _idle_check_loop(self) -> None:
        """Background thread: check if model has been idle too long."""
        while not self._idle_stop_event.wait(60):  # Check every 60 seconds
            if self.model is None:
                return
            idle_time = time.time() - self.last_used_time
            if idle_time >= self.idle_timeout_sec:
                logger.info(
                    "Model idle for %.0f seconds (timeout=%d), auto-unloading...",
                    idle_time,
                    self.idle_timeout_sec,
                )
                self.unload()
                return

    def unload(self) -> bool:
        """Release the model from GPU memory.

        Returns ``True`` if a model was previously loaded and has been
        released, ``False`` if there was nothing to unload.
        """
        # Stop idle check thread
        self._idle_stop_event.set()
        if self._idle_check_thread is not None:
            self._idle_check_thread.join(timeout=2)
            self._idle_check_thread = None

        if self.model is None:
            return False

        logger.info("Unloading ASR model …")
        del self.model
        self.model = None
        self.model_source = ""

        try:
            import torch

            if torch.cuda.is_available():
                torch.cuda.empty_cache()
        except ImportError:
            pass

        logger.info("Model unloaded, GPU cache cleared")
        return True

    # ── Inference ──────────────────────────────────────────────────

    def transcribe(
        self,
        audio_path: str,
        language: str | None = None,
    ) -> tuple[str, str | None]:
        """Transcribe a single WAV file.

        Parameters
        ----------
        audio_path : str
            Path to a WAV file (16 kHz mono recommended).
        language : str | None
            Language hint, e.g. ``"Chinese"`` or ``"English"``.
            ``None`` means auto-detect.

        Returns
        -------
        (text, language) : tuple[str, str | None]
            The recognised text and detected language.
        """
        if self.model is None:
            raise RuntimeError("Model is not loaded. Call load() first.")

        self.last_used_time = time.time()

        results = self.model.transcribe(
            audio=audio_path,
            language=language,
        )

        if not results:
            return "", None

        first = results[0]
        text = getattr(first, "text", "") or ""
        lang = getattr(first, "language", None)
        return text, lang


# ── Module-level singleton ─────────────────────────────────────────

#: Global engine instance used by ``server.py``.
engine_instance: Qwen3ASREngine = Qwen3ASREngine()
