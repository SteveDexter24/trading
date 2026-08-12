"""Train a tiny logistic baseline on synthetic quote features and export ONNX.

This is research scaffolding, not a production alpha model.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

import numpy as np
from skl2onnx import convert_sklearn
from skl2onnx.common.data_types import FloatTensorType
from sklearn.linear_model import LogisticRegression
from sklearn.model_selection import train_test_split
from sklearn.pipeline import Pipeline
from sklearn.preprocessing import StandardScaler


FEATURE_VERSION = "quote-momentum-v1"
MODEL_NAME = "offline-logistic-quote-momentum"
MODEL_VERSION = "v1"


def synthesize(n: int = 2_000, seed: int = 7) -> tuple[np.ndarray, np.ndarray]:
    rng = np.random.default_rng(seed)
    # Features: mid, spread_fraction, bid, ask, relative_spread_to_tick
    mid = rng.uniform(50.0, 200.0, size=n)
    spread = rng.uniform(0.0001, 0.02, size=n)
    bid = mid * (1.0 - spread / 2.0)
    ask = mid * (1.0 + spread / 2.0)
    relative_spread = (ask - bid) / 0.01
    x = np.column_stack([mid, spread, bid, ask, relative_spread]).astype(np.float32)
    # Label: tighter spreads and higher mids are more likely "positive".
    logits = 1.5 - 40.0 * spread + 0.005 * (mid - 100.0)
    probs = 1.0 / (1.0 + np.exp(-logits))
    y = (rng.uniform(size=n) < probs).astype(np.int64)
    return x, y


def train_and_export(output_dir: Path) -> Path:
    output_dir.mkdir(parents=True, exist_ok=True)
    x, y = synthesize()
    x_train, x_test, y_train, y_test = train_test_split(
        x, y, test_size=0.25, random_state=7, stratify=y
    )
    pipe = Pipeline(
        steps=[
            ("scale", StandardScaler()),
            (
                "clf",
                LogisticRegression(max_iter=500, random_state=7),
            ),
        ]
    )
    pipe.fit(x_train, y_train)
    accuracy = float(pipe.score(x_test, y_test))

    onnx_model = convert_sklearn(
        pipe,
        initial_types=[("features", FloatTensorType([None, x.shape[1]]))],
        target_opset=17,
    )
    artifact_path = output_dir / f"{MODEL_NAME}-{MODEL_VERSION}.onnx"
    artifact_path.write_bytes(onnx_model.SerializeToString())
    digest = hashlib.sha256(artifact_path.read_bytes()).hexdigest()

    metadata = {
        "name": MODEL_NAME,
        "version": MODEL_VERSION,
        "feature_version": FEATURE_VERSION,
        "artifact_digest": f"sha256:{digest}",
        "holdout_accuracy": accuracy,
        "feature_names": [
            "mid",
            "spread_fraction",
            "bid",
            "ask",
            "relative_spread_to_tick",
        ],
        "notes": [
            "Offline training only.",
            "Rust runtime scores frozen ONNX artifacts.",
            "Model outputs may propose signals but cannot size or approve orders.",
        ],
    }
    (output_dir / f"{MODEL_NAME}-{MODEL_VERSION}.json").write_text(
        json.dumps(metadata, indent=2),
        encoding="utf-8",
    )
    return artifact_path


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=Path("artifacts"),
        help="Directory for ONNX artifact and metadata JSON",
    )
    args = parser.parse_args()
    path = train_and_export(args.output_dir)
    print(f"exported {path}")


if __name__ == "__main__":
    main()
