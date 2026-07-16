"""Tests for the heavy model backends. Run: pytest heavy-backend/tests"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import data as D
import models as M


def test_dataset_shape():
    df = D.gen_dataset(n=600)
    assert len(df) == 600
    assert set(["program_id", "program_name", "age_band", "treatment", "completed"]).issubset(df.columns)
    # synthetic uplift should be positive on average for treated vs control
    tr = df[df.treatment == 1].completed.mean()
    co = df[df.treatment == 0].completed.mean()
    assert tr > co


def test_registry_contract():
    assert "xgb_completion" in M.REGISTRY
    assert "causal_uplift" in M.REGISTRY
    assert M.REGISTRY["causal_uplift"]["kind"] == "causal_tlearner"


def test_train_and_predict():
    df = D.gen_dataset(n=1200)
    pred, caus = M.train(df)
    sf = D.segment_frame(
        [{"age_band": "25-40", "program_name": "Food Aid"}] * 10, "25-40|Food Aid"
    )
    mean_p, lo, hi = M.predict_completion(pred, sf)
    assert 0.0 <= mean_p <= 1.0
    # percentile CI vs mean: allow tiny float slack on near-constant preds.
    assert lo - 1e-6 <= mean_p <= hi + 1e-6


def test_uplift_recovers_injected_effect():
    df = D.gen_dataset(n=3000, seed=7)
    _, caus = M.train(df)
    for _pid, pname, ab, _base, uplift in D.PROGRAMS:
        sf = D.segment_frame([{"age_band": ab, "program_name": pname}], f"{ab}|{pname}")
        est, _lo, _hi = M.predict_uplift(caus, sf)
        # Estimated uplift should be within 0.12 of the injected effect.
        assert abs(est - uplift) < 0.12


def test_refine_overlay():
    df = D.gen_dataset(n=1500)
    M.train(df)
    card = {
        "observed_completion": 0.45,
        "expected_completion": 0.50,
        "projected_completion": 0.55,
        "additional_completions": 10.0,
    }
    out = M.refine(card, [{"age_band": "25-40", "program_name": "Food Aid"}], "25-40|Food Aid")
    assert out["heavy_backend"].startswith("xgb_completion+causal_uplift")
    assert 0.0 <= out["projected_completion"] <= 1.0
    assert out["recommended_lever"] >= 0.0
    assert "n-propose" in out["node_narratives"]
