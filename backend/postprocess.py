"""
VoiceInput v2 — Text Post-processing

Provides transcription cleanup and domain-specific term correction.
The term dictionary maps common ASR misrecognitions of technical terms
to their correct spellings (e.g. ``"派森"`` → ``"Python"``).
"""

from __future__ import annotations

import logging
import re

logger = logging.getLogger(__name__)

# ── Term correction dictionary ─────────────────────────────────────
#
# Keys are common phonetic misrecognitions produced by Qwen3-ASR when
# the user speaks Chinese-mixed technical terms.  Values are the
# canonical English spellings.
#
# The dictionary is intentionally ordered longest-key-first so that
# multi-character matches take priority over shorter substrings.

TERM_CORRECTIONS: dict[str, str] = {
    # ── Programming languages & frameworks ──
    "派森": "Python",
    "派投迟": "PyTorch",
    "坦瑟福罗": "TensorFlow",
    "法斯特 API": "FastAPI",
    "法斯特api": "FastAPI",
    "法斯特艾皮艾": "FastAPI",
    "加戈": "Django",
    "弗拉斯克": "Flask",
    "诺恩派": "NumPy",
    "派丹提克": "Pandas",
    "瑞艾克特": "React",
    "vuejs": "Vue.js",
    "vue杰斯": "Vue.js",
    "诺德js": "Node.js",
    "诺德": "Node",
    "泰普斯科瑞普特": "TypeScript",
    "加瓦斯科瑞普特": "JavaScript",
    "泰瑞": "Tauri",
    "艾莱克纯": "Electron",
    "弗拉特": "Flutter",
    "达特": "Dart",
    "科特林": "Kotlin",
    "鲁斯特": "Rust",
    "戈朗": "Go",
    "拉拉维尔": "Laravel",
    "斯普林": "Spring",
    "派斯派克": "PySpark",
    # ── AI / ML terms ──
    "扣达": "CUDA",
    "扣问三": "Qwen3",
    "扣问": "Qwen",
    "斯凯莱恩": "scikit-learn",
    "马特波拉比利": "matplotlib",
    # ── Tools & platforms ──
    "吉特哈布": "GitHub",
    "吉特拉布": "GitLab",
    "吉特": "Git",
    "道客": "Docker",
    "库伯内特斯": "Kubernetes",
}

# Pre-sort by key length descending so longer matches are replaced first.
_SORTED_TERMS: list[tuple[str, str]] = sorted(
    TERM_CORRECTIONS.items(), key=lambda kv: len(kv[0]), reverse=True
)


def clean_transcription(text: str) -> str:
    """Remove common ASR artifacts from *text*.

    * Strips surrounding whitespace.
    * Collapses excessive word repetitions (``"the the the"`` → ``"the"``).
    * Removes spaces before common punctuation marks.
    * Collapses runs of spaces into a single space.
    """
    if not text:
        return ""

    text = text.strip()

    # Remove excessive repetition: "word, word, word" → "word"
    text = re.sub(r"(\b\w+\b)(?:,\s*\1){2,}", r"\1", text)

    # Remove space before punctuation
    text = re.sub(r"\s+([,.?!;:，。？！；：])", r"\1", text)

    # Collapse multiple spaces
    text = re.sub(r" {2,}", " ", text)

    return text.strip()


def apply_term_corrections(text: str) -> str:
    """Replace known ASR misrecognitions with canonical spellings.

    Iterates over :data:`TERM_CORRECTIONS` in longest-key-first order so
    that multi-character keys (e.g. ``"法斯特 API"``) are matched before
    shorter substrings.
    """
    if not text:
        return text

    for wrong, correct in _SORTED_TERMS:
        if wrong in text:
            text = text.replace(wrong, correct)

    return text


def apply_custom_term_corrections(
    text: str, custom_terms: dict[str, str] | None = None
) -> str:
    """Apply user-defined term corrections on top of built-in corrections.

    *custom_terms* is a mapping of ``{wrong_text: correct_text}``.
    Longer keys are replaced first to avoid partial overlap issues.
    """
    if not text or not custom_terms:
        return text

    sorted_custom = sorted(
        custom_terms.items(), key=lambda kv: len(kv[0]), reverse=True
    )
    for wrong, correct in sorted_custom:
        if wrong and wrong in text:
            text = text.replace(wrong, correct)

    return text
