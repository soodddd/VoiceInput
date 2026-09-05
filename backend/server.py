"""
VoiceInput v2 — FastAPI ASR Backend Server

Exposes a local HTTP API on ``127.0.0.1:PORT`` for the Tauri host
process.  All sensitive endpoints require an ``X-VoiceInput-Token``
header that matches the token passed via ``--token`` at startup.

Endpoints
~~~~~~~~~

* ``GET  /health``                  — liveness + GPU info (no token)
* ``GET  /model/status``            — model loaded / download state (token)
* ``POST /model/load``              — load model onto GPU (token)
* ``POST /model/unload``            — release GPU memory (token)
* ``POST /model/download``          — start model download (token)
* ``GET  /model/download/status``   — poll download progress (token)
* ``POST /transcribe``              — speech-to-text (token)
"""

from __future__ import annotations

import asyncio
import json
import logging
import os
import tempfile
import time

import soundfile as sf
import uvicorn
from contextlib import asynccontextmanager
from fastapi import (
    Depends,
    FastAPI,
    File,
    Form,
    Header,
    HTTPException,
    UploadFile,
)
from pydantic import BaseModel

import config
import model_manager
from asr_engine import engine_instance
from audio_utils import chunk_audio, load_and_preprocess
from postprocess import (
    apply_custom_term_corrections,
    apply_punctuation_mode,
    apply_term_corrections,
    apply_zh_en_spacing,
    clean_transcription,
    merge_transcription_chunks,
)
from version import APP_VERSION

# ── Logging ────────────────────────────────────────────────────────

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(name)s: %(message)s",
)
logger = logging.getLogger("server")

# ── Pydantic models ────────────────────────────────────────────────


class HealthResponse(BaseModel):
    version: str
    status: str
    model: str
    device: str
    model_loaded: bool
    gpu_available: bool
    max_new_tokens: int


class ModelStatusResponse(BaseModel):
    loaded: bool
    installed: bool
    model_path: str | None = None
    downloading: bool
    download_progress: float
    download_state: str
    download_message: str
    download_error: str | None = None
    strategy: str | None = None


class ModelLoadRequest(BaseModel):
    model_path: str | None = None
    # Kept for compatibility with v0.1.x clients.
    model_name: str | None = None


class ModelLoadResponse(BaseModel):
    status: str
    loaded: bool
    model: str | None = None


class DownloadRequest(BaseModel):
    source: str  # "modelscope" | "huggingface" | "local"
    local_path: str | None = None


class DownloadStatusResponse(BaseModel):
    state: str
    downloading: bool
    progress: float
    speed: float
    error: str | None = None
    message: str


class TranscribeResponse(BaseModel):
    text: str
    language: str | None = None
    duration_ms: float = 0
    process_ms: float = 0
    chunks: int = 1


_operation_lock = asyncio.Lock()
_MAX_UPLOAD_BYTES = 64 * 1024 * 1024


# ── Token verification ─────────────────────────────────────────────


async def verify_token(
    x_voiceinput_token: str | None = Header(None, alias="X-VoiceInput-Token"),
) -> None:
    """Dependency: reject requests whose token does not match."""
    if not config.TOKEN:
        # No token configured — deny everything (fail-closed)
        raise HTTPException(status_code=503, detail="Server token not configured")
    if x_voiceinput_token != config.TOKEN:
        raise HTTPException(status_code=403, detail="Invalid or missing token")


# ── Lifespan ───────────────────────────────────────────────────────


@asynccontextmanager
async def lifespan(app: FastAPI):
    """Startup / shutdown lifecycle.

    The model is **not** auto-loaded on startup — the frontend must
    explicitly call ``POST /model/load`` after ensuring the model is
    downloaded.  This allows the download-then-load first-run flow.
    """
    logger.info(
        "ASR backend starting on %s:%s (device=%s)",
        config.HOST,
        config.PORT,
        config.DEVICE,
    )
    gpu_ok = config.is_gpu_available()
    if not gpu_ok:
        logger.warning("CUDA GPU not available — /health will report gpu_available=false")
    else:
        logger.info("CUDA GPU detected")

    yield

    # Shutdown — release model if loaded
    if engine_instance.is_loaded:
        logger.info("Shutting down: unloading model …")
        engine_instance.unload()
    logger.info("ASR backend stopped")


# ── App ────────────────────────────────────────────────────────────

app = FastAPI(
    title="VoiceInput ASR Backend",
    version=APP_VERSION,
    lifespan=lifespan,
)

# ── Endpoints ──────────────────────────────────────────────────────


@app.get("/health", response_model=HealthResponse)
async def health() -> HealthResponse:
    """Liveness probe — no token required.

    Used by the Tauri host to poll until the backend is ready.
    """
    return HealthResponse(
        version=APP_VERSION,
        status="ok",
        model=config.MODEL_NAME,
        device=config.DEVICE,
        model_loaded=engine_instance.is_loaded,
        gpu_available=config.is_gpu_available(),
        max_new_tokens=config.MAX_NEW_TOKENS,
    )


@app.get("/model/status", response_model=ModelStatusResponse)
async def model_status(
    _token: None = Depends(verify_token),
) -> ModelStatusResponse:
    """Return model load state and download progress."""
    dl_status = model_manager.get_download_status()
    model_path = model_manager.get_downloaded_model_path()
    return ModelStatusResponse(
        loaded=engine_instance.is_loaded,
        installed=model_path is not None,
        model_path=model_path,
        downloading=bool(dl_status["downloading"]),
        download_progress=float(dl_status["progress"]),
        download_state=str(dl_status["state"]),
        download_message=str(dl_status["message"]),
        download_error=str(dl_status["error"]) if dl_status["error"] else None,
        strategy=config.MODEL_STRATEGY,
    )


class StrategyRequest(BaseModel):
    strategy: str  # "fast" | "balanced" | "accurate" | "memory"


@app.get("/model/strategy")
async def get_strategy(
    _token: None = Depends(verify_token),
) -> dict:
    """Return the current model inference strategy."""
    return {
        "strategy": config.MODEL_STRATEGY,
        "available": list(config.STRATEGY_PARAMS.keys()),
    }


@app.post("/model/strategy")
async def set_strategy(
    req: StrategyRequest,
    _token: None = Depends(verify_token),
) -> dict:
    """Set the model inference strategy.

    If the model is currently loaded it will be unloaded so that the
    next load uses the new strategy parameters (batch size, token
    limit, idle timeout).
    """
    if req.strategy not in config.STRATEGY_PARAMS:
        raise HTTPException(
            status_code=400,
            detail=f"Unknown strategy '{req.strategy}'. "
            f"Available: {list(config.STRATEGY_PARAMS.keys())}",
        )

    old = config.MODEL_STRATEGY
    if old != req.strategy:
        async with _operation_lock:
            if engine_instance.is_loaded:
                await asyncio.to_thread(engine_instance.unload)
            config.MODEL_STRATEGY = req.strategy
        logger.info("Model strategy changed: %s -> %s", old, req.strategy)

    return {"status": "ok", "strategy": req.strategy}


@app.post("/model/load", response_model=ModelLoadResponse)
async def model_load(
    req: ModelLoadRequest,
    _token: None = Depends(verify_token),
) -> ModelLoadResponse:
    """Load (or reload) the ASR model onto the GPU."""
    try:
        async with _operation_lock:
            await asyncio.to_thread(
                engine_instance.load,
                req.model_path or req.model_name,
            )
        return ModelLoadResponse(
            status="ok",
            loaded=True,
            model=engine_instance.model_source,
        )
    except Exception as exc:
        logger.error("Failed to load model: %s", exc, exc_info=True)
        raise HTTPException(status_code=500, detail=str(exc))


@app.post("/model/unload", response_model=ModelLoadResponse)
async def model_unload(
    _token: None = Depends(verify_token),
) -> ModelLoadResponse:
    """Release the model and free GPU memory."""
    async with _operation_lock:
        await asyncio.to_thread(engine_instance.unload)
    return ModelLoadResponse(
        status="ok",
        loaded=False,
        model=None,
    )


@app.post("/model/download")
async def model_download(
    req: DownloadRequest,
    _token: None = Depends(verify_token),
) -> dict:
    """Start a model download in the background.

    Returns immediately with ``{"status": "started"}``.  Poll
    ``GET /model/download/status`` for progress.
    """
    # If the model is already downloaded, short-circuit
    if req.source != "local" and model_manager.is_model_downloaded():
        return {
            "status": "already_downloaded",
            "downloading": False,
            "model_path": model_manager.get_downloaded_model_path(),
        }

    dl = model_manager.get_download_status()
    if dl["downloading"]:
        return {"status": "already_downloading", "downloading": True}

    try:
        started = model_manager.start_download(
            source=req.source,
            model_dir=str(config.MODEL_DIR),
            local_path=req.local_path,
        )
    except (ValueError, OSError) as exc:
        raise HTTPException(status_code=400, detail=str(exc)) from exc

    if started:
        return {"status": "started", "downloading": True}
    else:
        return {"status": "already_downloading", "downloading": True}


@app.get("/model/download/status", response_model=DownloadStatusResponse)
async def download_status(
    _token: None = Depends(verify_token),
) -> DownloadStatusResponse:
    """Poll the current model download progress."""
    dl = model_manager.get_download_status()
    return DownloadStatusResponse(
        state=str(dl["state"]),
        downloading=bool(dl["downloading"]),
        progress=float(dl["progress"]),
        speed=float(dl["speed"]),
        error=str(dl["error"]) if dl["error"] else None,
        message=str(dl["message"]),
    )


@app.post("/model/download/cancel")
async def cancel_download(
    _token: None = Depends(verify_token),
) -> dict:
    """Cancel an in-progress model download.

    Sets a flag that the download worker checks.  The actual SDK download
    may continue briefly in the background, but the UI will immediately
    reflect the cancelled state.  Restarting the download will resume from
    cached files.
    """
    requested = model_manager.cancel_download()
    return {"status": "cancelling" if requested else "idle"}


@app.post("/transcribe", response_model=TranscribeResponse)
async def transcribe(
    audio: UploadFile = File(...),
    language: str | None = Form(None),
    custom_terms: str | None = Form(None),
    punctuation_mode: str = Form("simple"),
    auto_space_zh_en: str = Form("true"),
    normalize_audio: str = Form("true"),
    trim_silence: str = Form("true"),
    silence_threshold_db: float = Form(-40.0),
    max_record_sec: int = Form(120),
    _token: None = Depends(verify_token),
) -> TranscribeResponse:
    """Validate, preprocess and transcribe one bounded local WAV upload."""
    if language == "auto" or not language:
        language = None

    if not engine_instance.is_loaded:
        raise HTTPException(status_code=503, detail="Model not loaded")

    wav_data = await audio.read(_MAX_UPLOAD_BYTES + 1)
    if not wav_data:
        raise HTTPException(status_code=400, detail="Empty audio file")
    if len(wav_data) > _MAX_UPLOAD_BYTES:
        raise HTTPException(status_code=413, detail="Audio upload exceeds 64 MiB")

    tmp_path: str | None = None
    try:
        tmp = tempfile.NamedTemporaryFile(suffix=".wav", delete=False)
        tmp.write(wav_data)
        tmp.close()
        tmp_path = tmp.name
        async with _operation_lock:
            return await asyncio.to_thread(
                _transcribe_file,
                tmp_path,
                language,
                custom_terms,
                punctuation_mode,
                _as_bool(auto_space_zh_en),
                _as_bool(normalize_audio),
                _as_bool(trim_silence),
                max(-80.0, min(-10.0, silence_threshold_db)),
                max(1, min(600, max_record_sec)),
            )
    except HTTPException:
        raise
    except ValueError as exc:
        raise HTTPException(status_code=400, detail=str(exc)) from exc
    except Exception as exc:
        logger.error("Transcription failed: %s", exc, exc_info=True)
        raise HTTPException(status_code=503, detail=str(exc)) from exc
    finally:
        if tmp_path:
            try:
                os.unlink(tmp_path)
            except OSError:
                pass


def _as_bool(value: str) -> bool:
    return value.strip().lower() in {"true", "1", "yes", "on"}


def _transcribe_file(
    tmp_path: str,
    language: str | None,
    custom_terms: str | None,
    punctuation_mode: str,
    auto_space: bool,
    normalize: bool,
    trim: bool,
    silence_threshold_db: float,
    max_record_sec: int,
) -> TranscribeResponse:
    """Blocking inference body, intentionally kept off FastAPI's event loop."""
    started = time.monotonic()
    chunk_paths: list[str] = []
    try:
        audio_np, sample_rate = load_and_preprocess(
            tmp_path,
            normalize=normalize,
            trim=trim,
            silence_threshold_db=silence_threshold_db,
        )
        duration_sec = len(audio_np) / sample_rate
        if duration_sec > max_record_sec + 1:
            raise ValueError(f"Audio exceeds the {max_record_sec}-second limit")

        chunks_data = chunk_audio(
            audio_np,
            sample_rate,
            threshold_sec=min(config.CHUNK_THRESHOLD_SEC, float(max_record_sec)),
        )
        texts: list[str] = []
        languages: list[str] = []
        for index, chunk in enumerate(chunks_data):
            chunk_path = f"{tmp_path}.chunk{index}.wav"
            sf.write(chunk_path, chunk, sample_rate)
            chunk_paths.append(chunk_path)
            text, detected_language = engine_instance.transcribe(chunk_path, language)
            if text:
                texts.append(text)
            if detected_language:
                languages.append(detected_language)

        merged = merge_transcription_chunks(texts)
        merged = apply_term_corrections(merged)
        if custom_terms:
            try:
                mapping = json.loads(custom_terms)
            except (json.JSONDecodeError, TypeError) as exc:
                raise ValueError("custom_terms must be a JSON object") from exc
            if not isinstance(mapping, dict):
                raise ValueError("custom_terms must be a JSON object")
            merged = apply_custom_term_corrections(merged, mapping)
        merged = apply_punctuation_mode(merged, mode=punctuation_mode)
        merged = apply_zh_en_spacing(merged, enabled=auto_space)

        detected = max(set(languages), key=languages.count) if languages else None
        process_ms = (time.monotonic() - started) * 1000
        logger.info(
            "Transcription complete: duration=%.0fms process=%.0fms chunks=%d language=%s",
            duration_sec * 1000,
            process_ms,
            len(chunks_data),
            detected,
        )
        return TranscribeResponse(
            text=clean_transcription(merged),
            language=detected,
            duration_ms=duration_sec * 1000,
            process_ms=process_ms,
            chunks=len(chunks_data),
        )
    finally:
        for chunk_path in chunk_paths:
            try:
                os.unlink(chunk_path)
            except OSError:
                pass
        if engine_instance.should_unload_after_use and engine_instance.is_loaded:
            engine_instance.unload()


# ── Direct-run fallback ────────────────────────────────────────────

if __name__ == "__main__":
    # When run directly (development), start with env-var config.
    # Production uses __main__.py which parses CLI arguments.
    uvicorn.run(
        app,
        host=config.HOST,
        port=config.PORT,
        log_level="info",
    )
