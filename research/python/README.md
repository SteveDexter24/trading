# Offline research training

This Python package trains and exports versioned ONNX models. It is intentionally
outside the Rust trading runtime.

## Rules

- Train offline only.
- Export frozen ONNX + metadata (`model version`, `feature version`, digest).
- Rust scores artifacts; it never trains.
- Predictions may propose signals only. Risk still approves and sizes orders.

## Setup

```sh
python -m venv .venv
source .venv/bin/activate
pip install -e .
```

## Train baseline and export ONNX

```sh
python -m trading_research_offline.train_baseline --output-dir artifacts
```

The exporter writes:

- `artifacts/offline-logistic-quote-momentum-v1.onnx`
- `artifacts/offline-logistic-quote-momentum-v1.json`

Point the Rust `OnnxPredictor` at that artifact after the ONNX runtime path is
enabled for production use. Until then, research backtests use
`DeterministicResearchModel`.
