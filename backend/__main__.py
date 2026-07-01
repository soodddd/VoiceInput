"""
VoiceInput v2 — ASR Backend Entry Point

Parses command-line arguments, initialises configuration, and starts
the uvicorn server.  This module is the PyInstaller entry point so
that the packaged executable accepts CLI flags::

    asr_backend.exe --token <UUID> --port 8765 --model-dir <path> --device cuda:0
"""

from __future__ import annotations

import argparse
import logging
import os
import sys
from pathlib import Path


def _default_model_dir() -> str:
    """Return the platform-appropriate default model directory."""
    local_app_data = os.environ.get(
        "LOCALAPPDATA",
        str(Path.home() / "AppData" / "Local"),
    )
    return str(Path(local_app_data) / "VoiceInput" / "models")


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    """Parse command-line arguments."""
    parser = argparse.ArgumentParser(
        prog="asr_backend",
        description="VoiceInput v2 — Local ASR Backend (Qwen3-ASR)",
    )
    parser.add_argument(
        "--token",
        required=True,
        help="Local security token for API authentication (required).",
    )
    parser.add_argument(
        "--port",
        type=int,
        default=8765,
        help="HTTP listen port (default: 8765).",
    )
    parser.add_argument(
        "--model-dir",
        type=str,
        default=None,
        help="Directory for storing downloaded models "
        "(default: %%LOCALAPPDATA%%\\VoiceInput\\models).",
    )
    parser.add_argument(
        "--device",
        type=str,
        default="cuda:0",
        help="Torch device for inference (default: cuda:0).",
    )
    parser.add_argument(
        "--model-strategy",
        type=str,
        default="balanced",
        choices=["fast", "balanced", "accurate"],
        help="Model inference strategy (default: balanced).",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> None:
    """Initialise config and launch the uvicorn server."""
    args = parse_args(argv)

    # Configure logging to stdout (Rust sidecar redirects to file)
    logging.basicConfig(
        level=logging.INFO,
        format="%(asctime)s [%(levelname)s] %(name)s: %(message)s",
        stream=sys.stdout,
    )
    logger = logging.getLogger("asr_backend")
    logger.info("Starting VoiceInput ASR backend v2.0")

    # Import after logging is configured so init messages are captured
    import config
    import uvicorn
    from server import app

    # Resolve model directory
    model_dir = args.model_dir if args.model_dir else _default_model_dir()

    # Populate runtime configuration from CLI args
    config.init_config(
        token=args.token,
        port=args.port,
        model_dir=model_dir,
        device=args.device,
        model_strategy=args.model_strategy,
    )

    logger.info(
        "Server starting on http://%s:%d  device=%s  model_dir=%s",
        config.HOST,
        config.PORT,
        config.DEVICE,
        config.MODEL_DIR,
    )

    # Start uvicorn — bind to 127.0.0.1 only (never expose to network)
    uvicorn.run(
        app,
        host=config.HOST,
        port=config.PORT,
        log_level="info",
    )


if __name__ == "__main__":
    main()
