"""Explicit training entrypoint (reproducible artifacts).

The server also lazy-trains on startup, but this script is the canonical way to
(re)generate the model artifacts and assert they learned the known uplifts.
"""
from __future__ import annotations

import data as D
import models as M


def main():
    df = D.gen_dataset()
    print(f"generated dataset n={len(df)}")
    pred, caus = M.train(df)
    # Sanity: recovered uplift per program should be close to the injected uplift.
    for pid, pname, ab, base, uplift in D.PROGRAMS:
        sf = D.segment_frame([{"age_band": ab, "program_name": pname}], f"{ab}|{pname}")
        est, lo, hi = M.predict_uplift(caus, sf)
        print(f"  {pname:12s} injected uplift=+{uplift*100:.0f}pp  estimated=+{est*100:.0f}pp (CI +{lo*100:.0f}–+{hi*100:.0f})")
    print("artifacts written to heavy-backend/artifacts/")


if __name__ == "__main__":
    main()
