"""Offline-only research training utilities.

Trained artifacts are exported to ONNX and scored by the Rust runtime.
This package must never be imported by the live trading binary.
"""

__version__ = "0.1.0"
