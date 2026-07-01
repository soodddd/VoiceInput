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

import json
import logging
import os
import tempfile
import time

import numpy as np
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
from fastapi.middleware.cors import CORSMiddleware
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
)

# ── Logging ────────────────────────────────────────────────────────

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(name)s: %(message)s",
)
logger = logging.getLogger("server")

# ── Pydantic models ────────────────────────────────────────────────


class HealthResponse(BaseModel):
    status: str
    model: str
    device: str
    model_loaded: bool
    gpu_available: bool
    max_new_tokens: int


class ModelStatusResponse(BaseModel):
    loaded: bool
    downloading: bool
    download_progress: float
    strategy: str | None = None


class ModelLoadRequest(BaseModel):
    model_name: str | None = None


class ModelLoadResponse(BaseModel):
    status: str
    loaded: bool
    model: str | None = None


class DownloadRequest(BaseModel):
    source: str  # "modelscope" | "huggingface" | "local"
    local_path: str | None = None


class DownloadStatusResponse(BaseModel):
    downloading: bool
    progress: float
    speed: float
    error: str | None = None


class TranscribeResponse(BaseModel):
    text: str
    language: str | None = None
    duration_ms: float = 0
    process_ms: float = 0
    chunks: int = 1


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
    version="2.0.0",
    lifespan=lifespan,
)

app.add_middleware(
    CORSMiddleware,
    allow_origins=[
        "http://127.0.0.1:1420",
        "http://localhost:1420",
        "tauri://localhost",
    ],
    allow_methods=["*"],
    allow_headers=["*"],
)


# ── Endpoints ──────────────────────────────────────────────────────


@app.get("/health", response_model=HealthResponse)
async def health() -> HealthResponse:
    """Liveness probe — no token required.

    Used by the Tauri host to poll until the backend is ready.
    """
    return HealthResponse(
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
    return ModelStatusResponse(
        loaded=engine_instance.is_loaded,
        downloading=bool(dl_status["downloading"]),
        download_progress=float(dl_status["progress"]),
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
    config.MODEL_STRATEGY = req.strategy
    logger.info("Model strategy changed: %s -> %s", old, req.strategy)

    # 如果模型已加载且策略发生变化，卸载模型以便下次加载使用新参数
    if old != req.strategy and engine_instance.is_loaded:
        logger.info("Unloading model to apply new strategy on next load")
        engine_instance.unload()

    return {"status": "ok", "strategy": req.strategy}


@app.post("/model/load", response_model=ModelLoadResponse)
async def model_load(
    req: ModelLoadRequest,
    _token: None = Depends(verify_token),
) -> ModelLoadResponse:
    """Load (or reload) the ASR model onto the GPU."""
    if engine_instance.is_loaded:
        engine_instance.unload()

    try:
        engine_instance.load(req.model_name)
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
    was_loaded = engine_instance.unload()
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
    if model_manager.is_model_downloaded():
        return {
            "status": "already_downloaded",
            "downloading": False,
            "model_path": model_manager.get_downloaded_model_path(),
        }

    dl = model_manager.get_download_status()
    if dl["downloading"]:
        return {"status": "already_downloading", "downloading": True}

    started = model_manager.start_download(
        source=req.source,
        model_dir=str(config.MODEL_DIR),
        local_path=req.local_path,
    )

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
        downloading=bool(dl["downloading"]),
        progress=float(dl["progress"]),
        speed=float(dl["speed"]),
        error=str(dl["error"]) if dl["error"] else None,
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
    model_manager.cancel_download()
    return {"status": "cancelled"}


@app.post("/transcribe", response_model=TranscribeResponse)
async def transcribe(
    audio: UploadFile = File(...),
    language: str | None = Form(None),
    custom_terms: str | None = Form(None),
    punctuation_mode: str = Form("simple"),
    auto_space_zh_en: str = Form("true"),
    _token: None = Depends(verify_token),
) -> TranscribeResponse:
    """Transcribe an uploaded WAV file.

    Parameters (multipart/form-data)
    --------------------------------
    audio : file
        WAV audio (any sample rate, mono or stereo).
    language : str | None
        Language hint (``"Chinese"``, ``"English"``, or ``None`` for auto).

    Returns
    -------
    TranscribeResponse
        Recognition result with timing and chunk metadata.
    """
    if language == "auto" or not language:
        language = None

    if not engine_instance.is_loaded:
        raise HTTPException(status_code=503, detail="Model not loaded")

    t0 = time.time()
    tmp_path: str | None = None
    temp_files: list[str] = []  # all temp files to clean up

    try:
        # 1. Save uploaded audio to a temp file
        wav_data = await audio.read()
        if not wav_data:
            raise HTTPException(status_code=400, detail="Empty audio file")

        tmp = tempfile.NamedTemporaryFile(suffix=".wav", delete=False)
        tmp.write(wav_data)
        tmp.close()
        tmp_path = tmp.name
        temp_files.append(tmp_path)

        # 2. Preprocess: load → mono → 16 kHz → normalise → trim silence
        audio_np, sr = load_and_preprocess(tmp_path)
        duration_ms = len(audio_np) / sr * 1000
        logger.info(
            "Audio received: %.0f ms (preprocessed), language_hint=%s",
            duration_ms,
            language,
        )

        # 3. Chunk if longer than threshold
        chunks_data = chunk_audio(audio_np, sr, threshold_sec=config.CHUNK_THRESHOLD_SEC)
        n_chunks = len(chunks_data)
        if n_chunks > 1:
            logger.info("Audio split into %d chunks", n_chunks)

        # 4. Transcribe each chunk
        all_texts: list[str] = []
        all_langs: list[str] = []

        for i, chunk in enumerate(chunks_data):
            if n_chunks > 1:
                logger.info(
                    "  Chunk %d/%d (%.1fs)",
                    i + 1,
                    n_chunks,
                    len(chunk) / sr,
                )

            # Write chunk to temp WAV
            chunk_path = tmp_path + f".chunk{i}.wav"
            sf.write(chunk_path, chunk, sr)
            temp_files.append(chunk_path)

            text_i, lang_i = engine_instance.transcribe(
                audio_path=chunk_path,
                language=language,
            )

            if text_i:
                all_texts.append(text_i)
            if lang_i:
                all_langs.append(lang_i)

        # 5. Merge → clean → term-correct → punctuation → spacing
        merged_text = " ".join(all_texts)
        merged_text = clean_transcription(merged_text)
        merged_text = apply_term_corrections(merged_text)

        # Apply user-defined custom term corrections
        if custom_terms:
            try:
                terms_dict = json.loads(custom_terms)
                merged_text = apply_custom_term_corrections(merged_text, terms_dict)
            except (json.JSONDecodeError, TypeError):
                logger.warning("Failed to parse custom_terms JSON")

        # P2-06: 标点模式处理
        merged_text = apply_punctuation_mode(merged_text, mode=punctuation_mode)

        # P2-07: 中英混排自动空格
        space_enabled = auto_space_zh_en.lower() in ("true", "1", "yes")
        merged_text = apply_zh_en_spacing(merged_text, enabled=space_enabled)

        # Determine dominant language
        if all_langs:
            # Most common language
            lang = max(set(all_langs), key=all_langs.count)
        else:
            lang = None

        process_ms = (time.time() - t0) * 1000
        logger.info(
            "Transcription complete [%s] %.0fms, %d chunk(s): %s",
            lang,
            process_ms,
            n_chunks,
            merged_text[:200],
        )

        # 省显存策略：每次识别完成后立即释放模型
        if engine_instance.should_unload_after_use and engine_instance.is_loaded:
            logger.info("Memory strategy: unloading model after transcription")
            engine_instance.unload()

        return TranscribeResponse(
            text=merged_text,
            language=lang,
            duration_ms=duration_ms,
            process_ms=process_ms,
            chunks=n_chunks,
        )

    except HTTPException:
        raise
    except Exception as exc:
        logger.error("Transcription failed: %s", exc, exc_info=True)
        raise HTTPException(status_code=503, detail=str(exc))

    finally:
        # 6. Robust temp file cleanup (None-check to avoid v1 bug)
        for fpath in temp_files:
            if fpath is not None:
                try:
                    if os.path.exists(fpath):
                        os.unlink(fpath)
                except OSError:
                    pass


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
