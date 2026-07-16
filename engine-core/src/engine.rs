use crate::models::*;
use crate::stats::*;
use crate::types::*;
use crate::govern;
use std::collections::HashMap;

fn lineage(run_id: &str, model_version: &str, holdout_mae: f64) -> Lineage {
    Lineage {
        run_id: run_id.into(),
        model_version: model_version.into(),
        holdout_mae,
        data_window: "all".into(),
    }
}

// The loop: diagnose -> propose -> validate -> simulate -> evaluate, assembled
// into the nested reasoning graph (ADR-0005 / 0014).
pub fn run_insight(q_json: &str) -> Result<InsightCard, String> {
    let q: Query = serde_json::from_str(q_json).map_err(|e| e.to_string())?;

    let prog_name: HashMap<&str, &str> = q
        .programs
        .iter()
        .map(|p| (p.program_id.as_str(), p.program_name.as_str()))
        .collect();

    let (age_band, prog) = q.segment.split_once('|').ok_or("bad segment format")?;

    let rows: Vec<f64> = q
        .beneficiaries
        .iter()
        .filter(|b| {
            b.age_band == age_band
                && prog_name.get(b.program_id.as_str()).map_or(false, |n| *n == prog)
        })
        .map(|b| b.completed)
        .collect();

    let base = q.skill_state.base_params.base_completion;
    let adj = q.skill_state.learned_adjustments.get(&q.segment).copied().unwrap_or(0.0);
    let (floor, ceil) = (q.skill_state.bounds.floor, q.skill_state.bounds.ceil);
    let run_id = format!("r-{}", q.segment.replace('|', "_"));

    let ctx = RunCtx {
        segment: q.segment.clone(),
        rows: rows.clone(),
        base,
        adj,
        floor,
        ceil,
        series: vec![],
    };

    // --- diagnose: empirical vs rules ---
    let seg = run_model("seg_mean", &ctx);
    let rules = run_model("rules_completion", &ctx);
    let gap = seg.value - rules.value;

    let diagnose = FindingNode {
        id: "n-diagnose".into(),
        kind: "why".into(),
        label: "Current completion vs expected".into(),
        segment: Some(q.segment.clone()),
        model_used: seg.model_id.clone(),
        value: seg.value,
        ci: [seg.ci_low, seg.ci_high],
        confidence: [seg.ci_low, seg.ci_high],
        evidence: vec![format!("beneficiaries n={}", rows.len()), "playbook: segment baseline".into()],
        narrative: Some(format!(
            "For {} the observed completion is {:.0}% (95% CI {:.0}%–{:.0}%), versus the rules-based expectation of {:.0}%.",
            q.segment, seg.value * 100.0, seg.ci_low * 100.0, seg.ci_high * 100.0, rules.value * 100.0
        )),
        approval: None,
        children: vec![FindingNode {
            id: "n-gap".into(),
            kind: "why".into(),
            label: "Expectation gap".into(),
            segment: Some(q.segment.clone()),
            model_used: rules.model_id.clone(),
            value: gap,
            ci: [gap, gap],
            confidence: [0.6, 0.9],
            evidence: vec!["seg_mean vs rules_completion".into()],
            narrative: Some(format!(
                "Observed is {:+.0} pp {} the {:.0}% expectation.",
                gap * 100.0,
                if gap < 0.0 { "below" } else { "above" },
                rules.value * 100.0
            )),
            approval: None,
            children: vec![],
            lineage: lineage(&run_id, &rules.version, 0.0),
        }],
        lineage: lineage(&run_id, &seg.version, 0.0),
    };

    // --- propose + simulate (Monte-Carlo over the corrective lever) ---
    // The lever is the uplift needed to close an underperformance gap, applied
    // on top of the *observed* completion (not the expected baseline — that
    // would be circular). When the segment already meets/exceeds expectation,
    // no corrective lever is proposed.
    let expected = rules.value;
    let lever = if gap < 0.0 {
        clamp(-gap, 0.0, ceil.max(0.0))
    } else {
        0.0
    };
    let sigma = 0.05;
    let n_sim = 2000u64;
    let mut rng = make_rng(0xC0FFEE);
    let mut sims: Vec<f64> = Vec::with_capacity(n_sim as usize);
    for _ in 0..n_sim {
        let s = lever + sigma * normal_sample(&mut rng);
        sims.push(clamp(seg.value + s, 0.05, 0.98));
    }
    sims.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let sim_mean = mean(&sims);
    let sim_lo = percentile(&sims, 0.025);
    let sim_hi = percentile(&sims, 0.975);
    let lift = sim_mean - seg.value; // improvement over current observed

    let mut propose = FindingNode {
        id: "n-propose".into(),
        kind: "simulate".into(),
        label: if lever > 0.0 {
            format!("Apply +{:.0} pp corrective lever", lever * 100.0)
        } else {
            "No corrective lever needed".into()
        },
        segment: Some(q.segment.clone()),
        model_used: "monte_carlo@1.0.0".into(),
        value: if lever > 0.0 { lift } else { 0.0 },
        ci: if lever > 0.0 { [sim_lo - seg.value, sim_hi - seg.value] } else { [0.0, 0.0] },
        confidence: [0.6, 0.9],
        evidence: vec![format!("{} simulations, sigma={}", n_sim, sigma), "clamped to [0.05,0.98]".into()],
        narrative: if lever > 0.0 {
            Some(format!(
                "Simulating a +{:.0} pp intervention over {} runs implies completion rises from {:.0}% to about {:.0}% (95% CI {:.0}%–{:.0}%), a lift of {:+.0} pp.",
                lever * 100.0, n_sim, seg.value * 100.0, sim_mean * 100.0, sim_lo * 100.0, sim_hi * 100.0, lift * 100.0
            ))
        } else {
            Some(format!(
                "The segment already meets or exceeds expectation ({:.0}% vs {:.0}% expected); no corrective lever proposed.",
                seg.value * 100.0, expected * 100.0
            ))
        },
        approval: if lever > 0.0 { Some("pending".into()) } else { Some("not_applicable".into()) },
        children: vec![],
        lineage: lineage(&run_id, "1.0.0", 0.0),
    };

    // --- validate: rules MAE on a disjoint holdout ---
    let mut train: Vec<f64> = Vec::new();
    let mut hold: Vec<f64> = Vec::new();
    for (idx, &val) in rows.iter().enumerate() {
        if idx % 2 == 0 {
            train.push(val);
        } else {
            hold.push(val);
        }
    }
    let rules_mae = if hold.is_empty() {
        0.0
    } else {
        let pred: Vec<f64> = hold.iter().map(|_| clamp(base + adj, 0.05, 0.98)).collect();
        mae(&hold, &pred)
    };

    let validate = FindingNode {
        id: "n-validate".into(),
        kind: "evidence".into(),
        label: "Holdout validation".into(),
        segment: Some(q.segment.clone()),
        model_used: "rules_completion@1.0.0".into(),
        value: rules_mae,
        ci: [rules_mae, rules_mae],
        confidence: [0.7, 0.95],
        evidence: vec![format!("holdout n={}", hold.len()), format!("train n={}", train.len())],
        narrative: Some(format!(
            "On a disjoint holdout (n={}) the rules model's MAE is {:.3}.",
            hold.len(), rules_mae
        )),
        approval: None,
        children: vec![],
        lineage: lineage(&run_id, "1.0.0", rules_mae),
    };

    // --- evaluate: DSL-style valuation (additional completions) ---
    let target_n = q.programs.iter().find(|p| p.program_name == prog).map(|p| p.target_n as f64).unwrap_or(0.0);
    let eval_value = (lift * target_n).max(0.0);
    let evaluate = FindingNode {
        id: "n-evaluate".into(),
        kind: "how".into(),
        label: "Estimated additional completions".into(),
        segment: Some(q.segment.clone()),
        model_used: "dsl_valuation@1.0.0".into(),
        value: eval_value,
        ci: [0.0, eval_value],
        confidence: [0.6, 0.9],
        evidence: vec!["cost_per_outcome = lift × target_n".into()],
        narrative: Some(format!("At target_n, the lever yields ~{:.0} additional completions.", eval_value)),
        approval: None,
        children: vec![],
        lineage: lineage(&run_id, "1.0.0", 0.0),
    };

    // --- learned playbook: is this recommendation already human-approved? ---
    let approved = q.playbook.approved_entries.get(&q.segment).cloned();
    let pb_version = q.playbook.version.max(1);
    let validated = approved.is_some();
    if validated {
        propose.approval = Some("approved".into());
    }

    let mut children = vec![diagnose, propose, validate, evaluate];
    if let Some(a) = &approved {
        children.insert(0, FindingNode {
            id: "n-playbook".into(),
            kind: "evidence".into(),
            label: format!("Validated in learned playbook (v{})", a.playbook_version),
            segment: Some(q.segment.clone()),
            model_used: "learned_playbook".into(),
            value: a.recommended_lever,
            ci: [a.recommended_lever, a.recommended_lever],
            confidence: [0.7, 0.95],
            evidence: vec![
                format!("approved_by {}", a.approved_by),
                format!("approved_at {}", a.approved_at),
                format!("holdout_mae {:.3}", a.holdout_mae),
            ],
            narrative: Some(format!(
                "This +{:.0} pp recommendation was human-approved and is now a versioned entry in the cross-tenant learned playbook (v{}). Projected completion {:.0}%, ~{:.0} additional completions.",
                a.recommended_lever * 100.0, a.playbook_version, a.projected_completion * 100.0, a.additional_completions
            )),
            approval: Some("approved".into()),
            children: vec![],
            lineage: lineage(&run_id, "playbook", a.holdout_mae),
        });
    }

    let headline = if validated {
        format!(
            "For {}, completion is {:.0}% vs {:.0}% expected — recommendation validated in learned playbook (v{}).",
            q.segment, seg.value * 100.0, expected * 100.0, pb_version
        )
    } else if lever > 0.0 {
        format!(
            "For {}, completion is {:.0}% vs {:.0}% expected — a +{:.0} pp lever lifts it to ~{:.0}%.",
            q.segment, seg.value * 100.0, expected * 100.0, lever * 100.0, sim_mean * 100.0
        )
    } else {
        format!(
            "For {}, completion is {:.0}% vs {:.0}% expected — already on track, no corrective lever needed.",
            q.segment, seg.value * 100.0, expected * 100.0
        )
    };

    // Card-level approval: approved if already in playbook, else pending only
    // when there is an actionable lever; otherwise not applicable.
    let card_approval = if validated {
        "approved"
    } else if lever > 0.0 {
        "pending"
    } else {
        "not_applicable"
    };

    // --- GOVERN: assemble the free-text scanned for direct identifiers ---
    let mut scan_text = String::new();
    scan_text.push_str(&q.question);
    scan_text.push('\n');
    scan_text.push_str(&headline);
    scan_text.push('\n');
    fn collect_text(n: &FindingNode, buf: &mut String) {
        buf.push_str(&n.label);
        buf.push('\n');
        if let Some(t) = &n.narrative {
            buf.push_str(t);
            buf.push('\n');
        }
        for e in &n.evidence {
            buf.push_str(e);
            buf.push('\n');
        }
        for c in &n.children {
            collect_text(c, buf);
        }
    }
    for c in &children {
        collect_text(c, &mut scan_text);
    }

    let mut card = InsightCard {
        question: q.question.clone(),
        headline,
        segment: q.segment.clone(),
        confidence: [0.6, 0.9],
        approval: card_approval.into(),
        lineage: CardLineage {
            run_id: run_id.clone(),
            skill_version: q.skill_manifest.version.clone(),
            playbook_version: format!("v{}", pb_version),
        },
        observed_completion: seg.value,
        expected_completion: expected,
        recommended_lever: lever,
        projected_completion: sim_mean,
        additional_completions: eval_value,
        holdout_mae: rules_mae,
        validated,
        children,
        govern: None,
        meta: HashMap::new(),
    };

    // PDPA-safe PrePublish verdict (ADR-0017): k-anonymity + consent + PII
    // redaction. Attached to every card so the published artifact is provably
    // governed, and the client can offer a redacted PDPA export.
    card.govern = Some(compute_govern(&card, rows.len(), &q.consent, &scan_text));
    Ok(card)
}

// Build the PDPA-safe PrePublish GOVERN report for a card.
fn compute_govern(
    card: &InsightCard,
    segment_n: usize,
    consent: &Option<ConsentManifest>,
    scan_text: &str,
) -> GovernReport {
    let k = govern::K_ANON;
    let pii = govern::scan_pii(scan_text);

    let k_ok = govern::k_anonymity_ok(segment_n);
    let c_ok = govern::consent_ok(consent, &card.segment);

    let mut checks: Vec<GovernCheck> = Vec::new();

    checks.push(GovernCheck {
        id: "k_anonymity".into(),
        name: "k-anonymity (segment size)".into(),
        status: if k_ok { "pass".into() } else { "fail".into() },
        detail: if k_ok {
            format!("segment n={} ≥ k={} — safe to publish aggregated stats", segment_n, k)
        } else {
            format!(
                "segment n={} < k={} — too few records, re-identification risk, publish blocked",
                segment_n, k
            )
        },
    });

    checks.push(GovernCheck {
        id: "consent".into(),
        name: "Consent manifest".into(),
        status: if c_ok { "pass".into() } else { "fail".into() },
        detail: match consent {
            None => "no consent manifest supplied — purpose-limitation check failed, publish blocked".into(),
            Some(c) => {
                if c_ok {
                    format!(
                        "consent v{} granted {} for '{}' covers this segment grain",
                        c.version, c.granted_at, c.purpose
                    )
                } else {
                    format!(
                        "consent present but purpose '{}' / grain not permitted for this segment — publish blocked",
                        c.purpose
                    )
                }
            }
        },
    });

    let card_json = serde_json::to_string(card).unwrap_or_default();
    let (redacted, pii_found) = govern::redact_pii(&card_json, &pii);
    checks.push(GovernCheck {
        id: "pii_redaction".into(),
        name: "PII redaction".into(),
        status: if pii_found { "warn".into() } else { "pass".into() },
        detail: if pii_found {
            format!(
                "{} direct identifier(s) found and redacted in the export: {}",
                pii.len(),
                pii.iter().map(|f| f.kind.clone()).collect::<Vec<_>>().join(", ")
            )
        } else {
            "no direct identifiers in published text".into()
        },
    });

    let blocked = !k_ok || !c_ok;
    let verdict = if blocked { "block" } else { "allow" }.into();

    let mut audit: Vec<String> = Vec::new();
    audit.push(format!("GOVERN PrePublish verdict={}", verdict));
    for ck in &checks {
        audit.push(format!("  - {} [{}]: {}", ck.name, ck.status, ck.detail));
    }
    if pii_found {
        for f in &pii {
            audit.push(format!("  - REDACTED {}: {}", f.kind, f.matched));
        }
    }

    GovernReport {
        verdict,
        segment_n,
        k,
        checks,
        redacted_card: if blocked { None } else { Some(redacted) },
        audit,
    }
}

