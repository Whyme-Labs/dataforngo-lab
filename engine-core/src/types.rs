use serde::{Serialize, Deserialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone)]
pub struct Prediction {
    pub value: f64,
    pub ci_low: f64,
    pub ci_high: f64,
    pub model_id: String,
    pub version: String,
    pub lineage: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Lineage {
    pub run_id: String,
    pub model_version: String,
    pub holdout_mae: f64,
    pub data_window: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct FindingNode {
    pub id: String,
    pub kind: String,        // why | how | simulate | evidence
    pub label: String,
    pub segment: Option<String>,
    pub model_used: String,
    pub value: f64,
    pub ci: [f64; 2],
    pub confidence: [f64; 2],
    pub evidence: Vec<String>,
    pub narrative: Option<String>,
    pub approval: Option<String>, // pending | approved | rejected | null
    pub children: Vec<FindingNode>,
    pub lineage: Lineage,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CardLineage {
    pub run_id: String,
    pub skill_version: String,
    pub playbook_version: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct InsightCard {
    pub question: String,
    pub headline: String,
    pub segment: String,
    pub confidence: [f64; 2],
    pub approval: String,
    pub lineage: CardLineage,
    // Structured, machine-readable summary so the approval/evolution path never
    // parses narrative prose (ADR-0012 human-gated playbook evolution).
    pub observed_completion: f64,
    pub expected_completion: f64,
    pub recommended_lever: f64,
    pub projected_completion: f64,
    pub additional_completions: f64,
    pub holdout_mae: f64,
    pub validated: bool,
    pub children: Vec<FindingNode>,
    // PDPA-safe PrePublish verdict (ADR-0017).
    #[serde(default)] pub govern: Option<GovernReport>,
    // Provenance + heavy-backend overlay metadata (ADR-0018). Empty when the
    // in-edge WASM core produced the estimates; populated when a heavy Python
    // model backend was dispatched and overlaid its estimates.
    #[serde(default)] pub meta: HashMap<String, String>,
}

// The consent manifest an org supplies before its data is analysed. PDPA
// purpose-limitation + grain control (ADR-0017, deepened GOVERN).
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct ConsentManifest {
    #[serde(default)] pub version: String,
    #[serde(default)] pub purpose: String, // "impact_reporting" | "research"
    #[serde(default)] pub granted_at: String,
    #[serde(default)] pub data_fields: Vec<String>,
    #[serde(default)] pub permitted_grains: Vec<String>, // e.g. ["18-24|Youth Skills"]
}

// One GOVERN check result. status: pass | warn | fail.
#[derive(Serialize, Deserialize, Clone)]
pub struct GovernCheck {
    pub id: String,
    pub name: String,
    pub status: String,
    pub detail: String,
}

// The PDPA-safe PrePublish verdict attached to every insight card.
#[derive(Serialize, Deserialize, Clone)]
pub struct GovernReport {
    pub verdict: String, // allow | block
    pub segment_n: usize,
    pub k: usize,
    pub checks: Vec<GovernCheck>,
    // JSON string of the card with direct identifiers redacted; None when blocked.
    pub redacted_card: Option<String>,
    pub audit: Vec<String>,
}

// A human-approved recommendation, persisted + versioned in the learned
// playbook (the cross-tenant moat). Written by the approval gate.
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct ApprovedEntry {
    #[serde(default)] pub segment: String,
    #[serde(default)] pub recommended_lever: f64,
    #[serde(default)] pub expected_completion: f64,
    #[serde(default)] pub projected_completion: f64,
    #[serde(default)] pub additional_completions: f64,
    #[serde(default)] pub approved_at: String,
    #[serde(default)] pub approved_by: String,
    #[serde(default)] pub run_id: String,
    #[serde(default)] pub holdout_mae: f64,
    #[serde(default)] pub playbook_version: i64,
}

// The evolving playbook passed into the engine. Unknown seed fields
// (playbook_version string, note, segments) are ignored by serde.
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Playbook {
    #[serde(default)] pub version: i64,
    #[serde(default)] pub approved_entries: HashMap<String, ApprovedEntry>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Program {
    pub program_id: String,
    pub program_name: String,
    pub age_band: String,
    #[serde(default)] pub start_date: String,
    #[serde(default)] pub end_date: String,
    #[serde(default)] pub target_n: i64,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Beneficiary {
    pub program_id: String,
    pub age_band: String,
    pub completed: f64,
    #[serde(default)] pub enrolled_date: String,
    #[serde(default)] pub outcome_notes: String,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct BaseParams {
    #[serde(default)] pub base_completion: f64,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Bounds {
    #[serde(default)] pub floor: f64,
    #[serde(default)] pub ceil: f64,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct SkillState {
    #[serde(default)] pub base_params: BaseParams,
    #[serde(default)] pub learned_adjustments: HashMap<String, f64>,
    #[serde(default)] pub bounds: Bounds,
    #[serde(default)] pub version: String,
    #[serde(default)] pub trained_on: String,
    #[serde(default)] pub holdout_mae: f64,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Constraints {
    #[serde(default)] pub holdout_min_n: i64,
    #[serde(default)] pub require_ci: bool,
    #[serde(default)] pub max_leverage: f64,
    #[serde(default)] pub min_confidence: f64,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct SkillManifest {
    pub skill_id: String,
    pub version: String,
    #[serde(default)] pub domain: String,
    #[serde(default)] pub goal: String,
    #[serde(default)] pub constraints: Constraints,
}

#[derive(Serialize, Deserialize)]
pub struct Query {
    pub question: String,
    pub segment: String,
    pub beneficiaries: Vec<Beneficiary>,
    pub programs: Vec<Program>,
    pub skill_manifest: SkillManifest,
    pub skill_state: SkillState,
    #[serde(default)] pub playbook: Playbook,
    #[serde(default)] pub consent: Option<ConsentManifest>,
}
