use crate::stats::*;
use crate::types::Prediction;

// Uniform context passed to every model. WASM = pure compute; callers inject
// any data the model needs (no I/O inside models).
pub struct RunCtx {
    pub segment: String,
    pub rows: Vec<f64>,
    pub base: f64,
    pub adj: f64,
    pub floor: f64,
    pub ceil: f64,
    pub series: Vec<f64>,
}

pub trait Model {
    fn id(&self) -> &str;
    fn version(&self) -> &str;
    fn predict(&self, ctx: &RunCtx) -> Prediction;
}

// Deterministic rules predictor (adapts the bakerysense rules idea).
pub struct RulesCompletion;
impl Model for RulesCompletion {
    fn id(&self) -> &str { "rules_completion" }
    fn version(&self) -> &str { "1.0.0" }
    fn predict(&self, ctx: &RunCtx) -> Prediction {
        let v = clamp(ctx.base + ctx.adj, 0.05, 0.98);
        Prediction {
            value: v, ci_low: v, ci_high: v,
            model_id: self.id().into(), version: self.version().into(),
            lineage: "rules@1.0.0".into(),
        }
    }
}

// Empirical per-segment mean completion. `completed` is a continuous proportion
// in [0,1], so we take the mean proportion with a normal (CLT) CI on the mean
// rather than a binomial count. This is correct for both continuous fractions
// and binary 0/1 outcomes (in which case it approximates the Wilson interval).
pub struct SegmentMean;
impl Model for SegmentMean {
    fn id(&self) -> &str { "seg_mean" }
    fn version(&self) -> &str { "1.0.0" }
    fn predict(&self, ctx: &RunCtx) -> Prediction {
        let n = ctx.rows.len() as f64;
        if n == 0.0 {
            return Prediction {
                value: 0.0, ci_low: 0.0, ci_high: 1.0,
                model_id: self.id().into(), version: self.version().into(),
                lineage: "seg_mean n=0".into(),
            };
        }
        let m = mean(&ctx.rows);
        let sd = std_dev(&ctx.rows);
        let se = sd / n.sqrt();
        let z = 1.96;
        let lo = clamp(m - z * se, 0.0, 1.0);
        let hi = clamp(m + z * se, 0.0, 1.0);
        Prediction {
            value: m, ci_low: lo, ci_high: hi,
            model_id: self.id().into(), version: self.version().into(),
            lineage: format!("seg_mean n={}", ctx.rows.len()),
        }
    }
}

// Seasonal-naive time-series forecast with a residual-std CI.
pub struct NaiveForecast;
impl Model for NaiveForecast {
    fn id(&self) -> &str { "naive_forecast" }
    fn version(&self) -> &str { "1.0.0" }
    fn predict(&self, ctx: &RunCtx) -> Prediction {
        if ctx.series.is_empty() {
            return Prediction {
                value: 0.0, ci_low: 0.0, ci_high: 1.0,
                model_id: self.id().into(), version: self.version().into(),
                lineage: "no series".into(),
            };
        }
        let last = *ctx.series.last().unwrap();
        let diffs: Vec<f64> = ctx.series.windows(2).map(|w| w[1] - w[0]).collect();
        let sd = std_dev(&diffs).max(1e-3);
        let z = 1.96;
        Prediction {
            value: last,
            ci_low: (last - z * sd).max(0.0),
            ci_high: (last + z * sd).min(1.0),
            model_id: self.id().into(), version: self.version().into(),
            lineage: "naive seasonal".into(),
        }
    }
}

// Uniform model dispatch (the registry). The TS edge performs any network I/O
// to heavy Python backends; this covers the in-edge models.
pub fn run_model(name: &str, ctx: &RunCtx) -> Prediction {
    match name {
        "rules_completion" => RulesCompletion.predict(ctx),
        "seg_mean" => SegmentMean.predict(ctx),
        "naive_forecast" => NaiveForecast.predict(ctx),
        _ => Prediction {
            value: 0.0, ci_low: 0.0, ci_high: 1.0,
            model_id: name.into(), version: "0".into(),
            lineage: "unknown model".into(),
        },
    }
}
