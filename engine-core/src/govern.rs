use serde::{Serialize, Deserialize};
use crate::types::ConsentManifest;

#[derive(Serialize, Deserialize)]
pub struct PiiFinding {
    pub kind: String,
    pub matched: String,
}

// Minimum segment size for safe publication of aggregated stats. Below this,
// even coarse aggregates can re-identify individuals in a small NGO cohort.
pub const K_ANON: usize = 5;

// Lightweight PrePublish PII scan (ADR-0011, deepening under ADR-0012).
// Approximate heuristics — enough to block obvious direct identifiers before
// any external (Infini) call. Not a substitute for full k-anonymity.
pub fn scan_pii(text: &str) -> Vec<PiiFinding> {
    let mut out: Vec<PiiFinding> = Vec::new();

    // emails
    for tok in text.split(|c: char| !c.is_alphanumeric() && c != '@' && c != '.' && c != '-' && c != '_') {
        if tok.contains('@') && tok.contains('.') && tok.len() > 5 {
            out.push(PiiFinding { kind: "email".into(), matched: tok.to_string() });
        }
    }

    // phones: runs of digits (across spaces / dashes / parens) of length >= 9
    let mut digits = String::new();
    let mut flush = |d: &mut String| {
        if d.len() >= 9 {
            out.push(PiiFinding { kind: "phone".into(), matched: d.clone() });
        }
        d.clear();
    };
    for c in text.chars() {
        if c.is_ascii_digit() {
            digits.push(c);
        } else if c.is_whitespace() || c == '-' || c == '(' || c == ')' {
            // accumulate across separators
        } else {
            flush(&mut digits);
        }
    }
    flush(&mut digits);

    // IC / id-like: a token with letters AND >= 6 digits, length <= 12
    for tok in text.split_whitespace() {
        let has_letter = tok.chars().any(|c| c.is_alphabetic());
        let digcount = tok.chars().filter(|c| c.is_ascii_digit()).count();
        if has_letter && digcount >= 6 && tok.len() <= 12 {
            out.push(PiiFinding { kind: "ic_or_id".into(), matched: tok.to_string() });
        }
    }

    out
}

// k-anonymity gate: a segment with fewer than K records is too small to
// publish safely (re-identification risk even for aggregates).
pub fn k_anonymity_ok(n: usize) -> bool {
    n >= K_ANON
}

// Consent / purpose-limitation gate (PDPA). The org must have granted consent
// for impact reporting (or research) and the requested segment grain must be
// within the permitted grains.
pub fn consent_ok(consent: &Option<ConsentManifest>, segment: &str) -> bool {
    match consent {
        None => false,
        Some(c) => {
            let purpose_ok = c.purpose == "impact_reporting" || c.purpose == "research";
            let grain_ok =
                c.permitted_grains.is_empty() || c.permitted_grains.iter().any(|g| g == segment);
            purpose_ok && grain_ok
        }
    }
}

// Replace every matched direct identifier with [REDACTED] in the card JSON so
// the published/exported artifact carries no PII. Returns (redacted, found_any).
pub fn redact_pii(json: &str, findings: &[PiiFinding]) -> (String, bool) {
    let mut out = json.to_string();
    let mut found = false;
    for f in findings {
        let tok = f.matched.trim();
        if tok.is_empty() {
            continue;
        }
        if out.contains(tok) {
            found = true;
            out = out.replace(tok, "[REDACTED]");
        }
    }
    (out, found)
}
