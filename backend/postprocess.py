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


# ── P2-06: Punctuation modes ──────────────────────────────────────


def apply_punctuation_mode(text: str, mode: str = "simple") -> str:
    """Apply punctuation processing based on the selected *mode*.

    Parameters
    ----------
    text : str
        The transcription text after term corrections.
    mode : str
        One of:
        * ``"raw"``          — return text as-is, no punctuation changes.
        * ``"simple"``       — ensure text ends with a period if no
          terminal punctuation is present; normalize spacing.
        * ``"input_method"`` — strip most punctuation for short input
          scenarios (chat, search boxes); keep only essential commas.

    Returns
    -------
    str
        The text with punctuation adjustments applied.
    """
    if not text:
        return text

    if mode == "raw":
        return text

    if mode == "input_method":
        # 移除句末标点（。.!?！？），保留逗号、顿号等中间标点
        text = re.sub(r"[。.!?！？]+$", "", text)
        # 移除句末多余空格
        return text.rstrip()

    # mode == "simple"
    # 如果文本末尾没有终止标点，补充句号
    stripped = text.rstrip()
    if stripped and stripped[-1] not in "。.!?！？；;":
        # 中文文本加句号，英文文本加句点
        if re.search(r"[\u4e00-\u9fff]", stripped):
            text = stripped + "。"
        else:
            text = stripped + "."
    return text


# ── P2-07: Chinese-English spacing ────────────────────────────────

# 中英文边界：中文字符与 ASCII 字母数字之间
_ZH_EN_BOUNDARY = re.compile(
    r"([\u4e00-\u9fff\u3400-\u4dbf\uf900-\ufaff])([A-Za-z0-9])"
)
_EN_ZH_BOUNDARY = re.compile(
    r"([A-Za-z0-9])([\u4e00-\u9fff\u3400-\u4dbf\uf900-\ufaff])"
)


def apply_zh_en_spacing(text: str, enabled: bool = True) -> str:
    """Insert a space between adjacent Chinese and English characters.

    Examples
    --------
    >>> apply_zh_en_spacing("用Python写代码")
    '用 Python 写代码'
    >>> apply_zh_en_spacing("hello世界")
    'hello 世界'

    Parameters
    ----------
    text : str
        Input transcription text.
    enabled : bool
        If ``False``, return text unchanged.
    """
    if not text or not enabled:
        return text

    # 中→英边界加空格
    text = _ZH_EN_BOUNDARY.sub(r"\1 \2", text)
    # 英→中边界加空格
    text = _EN_ZH_BOUNDARY.sub(r"\1 \2", text)

    # 避免标点前后的多余空格
    text = re.sub(r"\s+([，。！？；：、,\.!?;:])", r"\1", text)
    text = re.sub(r"([（(\[【「『])\s+", r"\1", text)
    text = re.sub(r"\s+([）)\]】」』])", r"\1", text)

    # 合并连续空格
    text = re.sub(r" {2,}", " ", text)

    return text.strip()
