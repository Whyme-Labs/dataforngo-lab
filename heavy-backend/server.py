"""FastAPI service exposing the heavy model backends (ADR-0018).

Run locally:  uvicorn server:app --port 8000
The Worker dispatches to /refine (and /registry) when HEAVY_BACKEND_URL is set.
"""
from __future__ import annotations

import time

from fastapi import FastAPI, HTTPException
from pydantic import BaseModel

import data as D
import models as M

app = FastAPI(title="DataForNGO Heavy Model Backend", version="1.0.0")


class SegReq(BaseModel):
    segment: str
    beneficiaries: list[dict] = []


class RefineReq(BaseModel):
    segment: str
    beneficiaries: list[dict] = []
    card: dict
    target_n: float = 0.0


class NarrateReq(BaseModel):
    segment: str
    observed: float = 0.0
    expected: float = 0.0
    projected: float = 0.0
    additional: float = 0.0


@app.on_event("startup")
def _load():
    # Always train fresh at startup (fast, deterministic, no stale-cache risk).
    # The heavy models are tiny XGBoost trees; training on the synthetic NGO
    # data takes <2s. Endpoints read from these globals.
    global PRED, CAUS
    PRED, CAUS = M.train(D.gen_dataset())


PRED = None
CAUS = None


@app.get("/health")
def health():
    return {"status": "ok", "ts": time.time()}


@app.get("/registry")
def registry():
    return {"registry": M.REGISTRY}


@app.post("/predict")
def predict(req: SegReq):
    sf = M.D.segment_frame(req.beneficiaries, req.segment)
    mean, lo, hi = M.predict_completion(PRED, sf)
    return {"segment": req.segment, "mean_p": mean, "ci": [lo, hi]}


@app.post("/uplift")
def uplift(req: SegReq):
    sf = M.D.segment_frame(req.beneficiaries, req.segment)
    mean, lo, hi = M.predict_uplift(CAUS, sf)
    return {"segment": req.segment, "uplift": mean, "ci": [lo, hi]}


@app.post("/narrate")
def narrate(req: NarrateReq):
    return {
        "segment": req.segment,
        "narratives": {
            "n-propose": (
                f"Heavy causal model estimates the support lever lifts completion "
                f"to ~{req.projected*100:.0f}% for {req.segment}."
            ),
            "n-evaluate": (
                f"At target_n, the data-driven estimate implies "
                f"~{req.additional:.0f} additional completions."
            ),
        },
    }


@app.post("/refine")
def refine(req: RefineReq):
    card = dict(req.card)
    card["target_n"] = req.target_n
    try:
        return M.refine_models(card, req.beneficiaries, req.segment, PRED, CAUS)
    except Exception as e:  # never let a heavy-backend error break the Worker
        raise HTTPException(status_code=502, detail=f"refine failed: {e}")
