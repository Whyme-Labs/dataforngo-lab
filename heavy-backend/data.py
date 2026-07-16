"""Reproducible synthetic NGO training data for the heavy model backends.

Mirrors the distributions used by the in-edge seed (contest-app/worker/src/config.ts)
so the XGBoost predictor and causal uplift model learn on the same world the demo
shows. No external data is required.
"""
from __future__ import annotations

import numpy as np
import pandas as pd

# (program_id, program_name, age_band, base_completion, treatment_uplift_pp)
PROGRAMS = [
    ("p1", "Youth Skills", "18-24", 0.75, 0.18),
    ("p2", "Food Aid", "25-40", 0.45, 0.27),
    ("p3", "Elderly Care", "60+", 0.60, 0.12),
]


def gen_dataset(n: int = 6000, seed: int = 20260715) -> pd.DataFrame:
    rng = np.random.default_rng(seed)
    rows = []
    per = n // len(PROGRAMS)
    for pid, pname, ab, base, uplift in PROGRAMS:
        for _ in range(per):
            treatment = int(rng.random() < 0.5)
            noise = rng.normal(0.0, 0.12)
            # Probability of completion under this arm.
            p = float(np.clip(base + (uplift if treatment else 0.0) + noise, 0.02, 0.98))
            # Probit-style draw of the binary outcome.
            completed = 1.0 if p > rng.random() else 0.0
            enrolled_days = int(rng.integers(30, 400))
            rows.append(
                dict(
                    program_id=pid,
                    program_name=pname,
                    age_band=ab,
                    treatment=treatment,
                    enrolled_days=enrolled_days,
                    completed=completed,
                )
            )
    return pd.DataFrame(rows)


# program_name -> program_id (the worker's beneficiaries are keyed by id).
PROG_ID = {name: pid for pid, name, _ab, _base, _uplift in PROGRAMS}


def segment_frame(beneficiaries: list[dict], segment: str) -> pd.DataFrame:
    """Project the worker's beneficiary JSON into the model's feature frame.

    The worker passes the *observed* beneficiaries for the chosen segment; we
    attach a synthetic `treatment` column (absent in the raw NGO data) by
    assuming the support lever has NOT yet been applied to the observed cohort
    (treatment=0), which is exactly the control arm the uplift model compares
    against. This keeps the heavy backend honest: it estimates the *effect* of
    applying the lever, it does not assume it was already applied.

    Beneficiaries may be keyed by `program_id` (the worker) or `program_name`
    (tests); both are accepted.
    """
    age_band, prog = (segment.split("|") + ["", ""])[:2]
    pid = PROG_ID.get(prog, "")
    rows = []
    for b in beneficiaries:
        if b.get("age_band") != age_band:
            continue
        bpid = b.get("program_id")
        bpname = b.get("program_name")
        if pid and bpid and bpid != pid:
            continue
        if not bpid and bpname and bpname != prog:
            continue
        rows.append(
            dict(
                program_name=prog or bpname or "",
                age_band=b.get("age_band", age_band),
                enrolled_days=int(b.get("enrolled_days", 180)),
                treatment=0,
            )
        )
    return pd.DataFrame(rows)
