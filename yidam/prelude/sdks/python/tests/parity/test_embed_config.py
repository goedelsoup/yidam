"""Python runner for the embedding reproducibility contract
(parity/fixtures/embed_config/). Mirrors the Rust reference runner in
yidam/cli/tests/embed_parity.rs.

Downloads model weights on first run, so it only executes when
YIDAM_EMBED_PARITY=1 is set and sentence-transformers is installed
(``uv run --extra embed pytest``; see the `embed-parity` mise task).

sentence-transformers cannot load the quantized ONNX export the Rust
reference uses, so the runner maps the contract model_id to the original
fp32 ``sentence-transformers/`` repo and applies the fixture's
``[known_delta.sentence-transformers]`` tolerance instead of the strict
cross-ONNX tolerance.
"""

import math
import os
import tomllib
from pathlib import Path

import pytest

FIXTURES_DIR = (
    Path(__file__).parent.parent.parent.parent / "parity" / "fixtures" / "embed_config"
)

MODEL_ID_MAP = {
    "Xenova/all-MiniLM-L6-v2": "sentence-transformers/all-MiniLM-L6-v2",
}

enabled = os.environ.get("YIDAM_EMBED_PARITY") == "1"


def load_fixtures() -> list[tuple[str, dict]]:
    if not FIXTURES_DIR.exists():
        return []
    out = []
    for p in sorted(FIXTURES_DIR.glob("*.toml")):
        with open(p, "rb") as f:
            out.append((p.name, tomllib.load(f)))
    return out


@pytest.mark.skipif(not enabled, reason="set YIDAM_EMBED_PARITY=1 to run")
def test_embed_config_parity():
    st = pytest.importorskip("sentence_transformers")

    fixtures = load_fixtures()
    assert fixtures, "no embed_config fixtures"

    for name, fx in fixtures:
        inp = fx["input"]
        exp = fx["expected"]
        tolerance = (
            fx.get("known_delta", {})
            .get("sentence-transformers", {})
            .get("tolerance", exp["tolerance"])
        )

        model_id = MODEL_ID_MAP.get(inp["model_id"], inp["model_id"])
        model = st.SentenceTransformer(model_id)
        vector = model.encode(inp["text"], normalize_embeddings=inp["normalize"])

        assert len(vector) == exp["embedding_dim"], "embedding_dim"

        norm = math.sqrt(sum(float(x) ** 2 for x in vector))
        assert abs(norm - 1.0) < 1e-3, f"vector should be L2-normalized, norm = {norm}"

        prefix = [float(x) for x in vector[: len(exp["prefix"])]]
        for i, (a, e) in enumerate(zip(prefix, exp["prefix"])):
            assert abs(a - e) <= tolerance, (
                f"{name}: prefix[{i}] = {a}, expected {e} "
                f"(tolerance {tolerance})\nfull actual prefix: {prefix}"
            )
