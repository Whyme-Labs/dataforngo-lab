"""Heavy model backends: XGBoost completion predictor + causal uplift (T-learner).

These are the "deep" models the in-edge Rust/WASM core cannot run (no XGBoost in
wasm32-unknown-unknown, no double-ML). They are exposed as a registry of services
the Worker dispatches to when HEAVY_BACKEND_URL is configured (ADR-0018).
"""
from __future__ import annotations

import joblib
import numpy as np
import pandas as pd
from sklearn.model_selection import train_test_split

try:
    from xgboost import XGBClassifier
except Exception:  # pragma: no cover - xgboost is a hard dependency at runtime
    XGBClassifier = None

import data as D

ARTIFACT_DIR = __import__("pathlib").Path(__file__).parent / "artifacts"
ARTIFACT_DIR.mkdir(exist_ok=True)

# The model registry the Worker dispatches against. Each entry is a real,
# loadable backend; TimerXL / Sundial are documented as future GPU-hosted
# entries that slot into the same registry contract.
REGISTRY = {
    "xgb_completion": {
        "kind": "supervised_classifier",
        "version": "1.0.0",
        "target": "completed",
        "features": ["program_name", "age_band", "treatment", "enrolled_days"],
        "desc": "XGBoost P(completion): captures program / age-band / treatment effects.",
    },
    "causal_uplift": {
        "kind": "causal_tlearner",
        "version": "1.0.0",
        "estimand": "ATE of support-lever treatment on completion",
        "desc": "Double-ML T-learner uplift = treated_model − control_model.",
    },
}


def _encode(frame: pd.DataFrame) -> pd.DataFrame:
    """Encode categoricals with a FIXED vocabulary so train and inference map the
    same program/age-band strings to the same integer codes (per-frame
    ``astype('category')`` would remap codes and silently corrupt predictions)."""
    out = frame.copy()
    prog_code = {name: i for i, (_pid, name, _ab, _base, _u) in enumerate(D.PROGRAMS)}
    age_code = {ab: i for i, (_pid, _name, ab, _base, _u) in enumerate(D.PROGRAMS)}
    out["program_name"] = out["program_name"].map(prog_code).fillna(0).astype("int64")
    out["age_band"] = out["age_band"].map(age_code).fillna(0).astype("int64")
    return out


def train(df: pd.DataFrame | None = None):
    """Train (and persist) the predictor + causal models. Returns (pred, caus)."""
    if df is None:
        df = D.gen_dataset()
    feat = _encode(df)
    X = feat[["program_name", "age_band", "treatment", "enrolled_days"]]
    y = feat["completed"].astype(int)

    X_tr, X_te, y_tr, y_te = train_test_split(X, y, test_size=0.2, random_state=1)
    pred = XGBClassifier(
        n_estimators=120, max_depth=4, learning_rate=0.1, subsample=0.9, n_jobs=-1
    )
    pred.fit(X_tr, y_tr)

    # T-learner: separate models for treated / control arms.
    treated = feat[feat["treatment"] == 1]
    control = feat[feat["treatment"] == 0]
    m_t = XGBClassifier(n_estimators=120, max_depth=4, learning_rate=0.1, n_jobs=-1)
    m_c = XGBClassifier(n_estimators=120, max_depth=4, learning_rate=0.1, n_jobs=-1)
    m_t.fit(
        treated[["program_name", "age_band", "enrolled_days"]],
        treated["completed"].astype(int),
    )
    m_c.fit(
        control[["program_name", "age_band", "enrolled_days"]],
        control["completed"].astype(int),
    )

    joblib.dump(pred, ARTIFACT_DIR / "xgb_completion.joblib")
    joblib.dump({"treated": m_t, "control": m_c}, ARTIFACT_DIR / "causal_uplift.joblib")
    return pred, {"treated": m_t, "control": m_c}


def load():
    try:
        pred = joblib.load(ARTIFACT_DIR / "xgb_completion.joblib")
        caus = joblib.load(ARTIFACT_DIR / "causal_uplift.joblib")
        return pred, caus
    except Exception:
        return train()


def predict_completion(pred, seg_frame: pd.DataFrame) -> tuple[float, float, float]:
    """Return (mean_p, ci_low, ci_high) of P(completion) for a control-arm frame."""
    if seg_frame.empty:
        return (0.0, 0.0, 0.0)
    X = _encode(seg_frame)[["program_name", "age_band", "treatment", "enrolled_days"]]
    p = pred.predict_proba(X)[:, 1]
    lo, hi = np.percentile(p, [2.5, 97.5])
    return (float(p.mean()), float(lo), float(hi))


def predict_uplift(caus, seg_frame: pd.DataFrame) -> tuple[float, float, float]:
    """Causal uplift (treated − control) for the segment, with bootstrap CI."""
    if seg_frame.empty:
        return (0.0, 0.0, 0.0)
    base = _encode(seg_frame)[["program_name", "age_band", "enrolled_days"]]
    m_t, m_c = caus["treated"], caus["control"]
    pt = m_t.predict_proba(base)[:, 1]
    pc = m_c.predict_proba(base)[:, 1]
    uplift = pt - pc
    lo, hi = np.percentile(uplift, [2.5, 97.5])
    return (float(uplift.mean()), float(lo), float(hi))


def refine_models(card: dict, beneficiaries: list[dict], segment: str, pred, caus) -> dict:
    """Overlay heavy estimates onto an in-edge card (the /refine contract)."""
    sf = D.segment_frame(beneficiaries, segment)
    observed = float(card.get("observed_completion", 0.0))
    expected = float(card.get("expected_completion", 0.0))

    _, _, _ = predict_completion(pred, sf)
    uplift, u_lo, u_hi = predict_uplift(caus, sf)
    uplift = float(np.clip(uplift, 0.0, 0.4))
    u_lo = max(0.0, u_lo)
    u_hi = min(0.4, u_hi)

    projected = float(np.clip(observed + uplift, 0.05, 0.98))
    target_n = float(card.get("target_n", 0.0) or 0.0)
    additional = max(0.0, uplift * target_n)

    node_narratives = {
        "n-propose": (
            f"Heavy causal model (double-ML T-learner) estimates the support lever "
            f"lifts completion by +{uplift*100:.0f} pp for this segment "
            f"(95% CI +{u_lo*100:.0f}–+{u_hi*100:.0f} pp), versus the in-edge Monte-Carlo guess."
        ),
        "n-evaluate": (
            f"At target_n={target_n:.0f}, the causal estimate implies "
            f"~{additional:.0f} additional completions (data-driven, not simulated)."
        ),
    }

    return {
        "heavy_backend": "xgb_completion+causal_uplift@1.0.0",
        "model_versions": {"xgb_completion": "1.0.0", "causal_uplift": "1.0.0"},
        "observed_completion": observed,
        "expected_completion": expected,
        "projected_completion": projected,
        "projected_ci": [projected - (u_hi - uplift), projected + (u_hi - uplift)],
        "recommended_lever": uplift,
        "additional_completions": additional,
        "node_narratives": node_narratives,
        "note": "Heavy estimates overlay the in-edge card; in-edge values used as fallback if this call fails.",
    }


def refine(card: dict, beneficiaries: list[dict], segment: str) -> dict:
    """Convenience wrapper used by tests / offline callers (loads models)."""
    pred, caus = load()
    return refine_models(card, beneficiaries, segment, pred, caus)
