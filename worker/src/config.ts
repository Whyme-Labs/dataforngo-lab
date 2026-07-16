// Skill manifest + state + playbook (ADR-0013) and bundled synthetic data
// (ADR-0003 / 0012). The playbook is pre-loaded + versioned; with ADR-0012 it
// will also persist and evolve (human-gated) once storage is wired.
import skillManifest from "../../skills/youth_skills_impact.manifest.json";
import skillState from "../../skills/youth_skills_impact.state.json";
import playbook from "../../playbook/playbook.v1.json";

export const SKILL_MANIFEST = skillManifest;
export const SKILL_STATE = skillState;
export const PLAYBOOK = playbook;

// Consent manifest (PDPA purpose-limitation + grain control, ADR-0017). The org
// grants consent for impact reporting on the listed segment grains. Send null to
// simulate a withdrawn/never-granted consent (publish blocked).
export const CONSENT = {
  version: "1.0",
  purpose: "impact_reporting",
  granted_at: "2026-07-01",
  data_fields: ["age_band", "program_name", "completed", "target_n", "enrolled_date"],
  permitted_grains: [
    "18-24|Youth Skills",
    "25-40|Food Aid",
    "60+|Elderly Care",
    "41-59|Pilot Micro",
  ],
};

// Deterministic synthetic NGO data so the demo runs with zero upload.
function lcg(seed: number): () => number {
  let s = seed >>> 0;
  return () => {
    s = (s * 1664525 + 1013904223) >>> 0;
    return s / 0xffffffff;
  };
}

interface Program {
  program_id: string;
  program_name: string;
  age_band: string;
  start_date: string;
  end_date: string;
  target_n: number;
}

interface Beneficiary {
  program_id: string;
  age_band: string;
  completed: number;
  enrolled_date: string;
  outcome_notes: string;
}

function genBeneficiaries(): Beneficiary[] {
  const rng = lcg(0x1234abcd);
  const segs = [
    { pid: "p1", name: "Youth Skills", ab: "18-24", base: 0.75 },
    { pid: "p2", name: "Food Aid", ab: "25-40", base: 0.45 },
    { pid: "p3", name: "Elderly Care", ab: "60+", base: 0.6 },
    // Tiny pilot cohort — only 3 records, below k=5, to demonstrate the
    // GOVERN k-anonymity publish block (ADR-0017).
    { pid: "p4", name: "Pilot Micro", ab: "41-59", base: 0.5, n: 3 },
  ];
  const rows: Beneficiary[] = [];
  for (const sg of segs) {
    const count = (sg as any).n ?? 80;
    for (let i = 0; i < count; i++) {
      const noise = (rng() - 0.5) * 0.4;
      const c = Math.min(0.99, Math.max(0.01, sg.base + noise));
      rows.push({
        program_id: sg.pid,
        age_band: sg.ab,
        completed: Math.round(c * 100) / 100,
        enrolled_date: "2025-01-01",
        outcome_notes: "",
      });
    }
  }
  return rows;
}

export const SAMPLE = {
  programs: [
    { program_id: "p1", program_name: "Youth Skills", age_band: "18-24", start_date: "2025-01-01", end_date: "2025-12-31", target_n: 120 },
    { program_id: "p2", program_name: "Food Aid", age_band: "25-40", start_date: "2025-01-01", end_date: "2025-12-31", target_n: 200 },
    { program_id: "p3", program_name: "Elderly Care", age_band: "60+", start_date: "2025-01-01", end_date: "2025-12-31", target_n: 80 },
    { program_id: "p4", program_name: "Pilot Micro", age_band: "41-59", start_date: "2025-01-01", end_date: "2025-12-31", target_n: 10 },
  ] as Program[],
  beneficiaries: genBeneficiaries(),
};
