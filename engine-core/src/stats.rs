// Statistical helpers: CIs, MAE, clamp, seeded RNG, normal sampling.

pub fn mean(xs: &[f64]) -> f64 {
    if xs.is_empty() { return 0.0; }
    xs.iter().sum::<f64>() / xs.len() as f64
}

pub fn variance(xs: &[f64]) -> f64 {
    if xs.len() < 2 { return 0.0; }
    let m = mean(xs);
    xs.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (xs.len() - 1) as f64
}

pub fn std_dev(xs: &[f64]) -> f64 { variance(xs).sqrt() }

pub fn mae(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len().min(b.len());
    if n == 0 { return 0.0; }
    let mut s = 0.0;
    for i in 0..n { s += (a[i] - b[i]).abs(); }
    s / n as f64
}

pub fn clamp(x: f64, lo: f64, hi: f64) -> f64 { x.max(lo).min(hi) }

// Acklam inverse standard-normal CDF (rational approximation).
fn ppf_low(q: f64) -> f64 {
    let c = [-7.784894002430293e-03, -3.223964580411365e-01, -2.400758277161838e+00, -2.549732539343734e+00, 4.374664141464968e+00, 2.938163982698783e+00];
    let d = [7.784695709041462e-03, 3.224671290700398e-01, 2.445134137142996e+00, 3.754408661907416e+00];
    let num = (((((c[0]*q+c[1])*q+c[2])*q+c[3])*q+c[4])*q+c[5]);
    let den = (((((d[0]*q+d[1])*q+d[2])*q+d[3])*q+1.0));
    num / den
}

fn ppf_mid(q: f64) -> f64 {
    let a = [-3.969683028665376e+01, 2.209460984245205e+02, -2.759285104469687e+02, 1.383577518672690e+02, -3.066479806614716e+01, 2.506628277459239e+00];
    let b = [-5.447609879822406e+01, 1.615858368580409e+02, -1.556989798598866e+02, 6.680131188771972e+01, -1.328068155288572e+01];
    let r = q * q;
    let num = (((((a[0]*r+a[1])*r+a[2])*r+a[3])*r+a[4])*r+a[5]) * q;
    let den = (((((b[0]*r+b[1])*r+b[2])*r+b[3])*r+b[4])*r+1.0);
    num / den
}

pub fn norm_ppf(p: f64) -> f64 {
    if p <= 0.0 { return f64::NEG_INFINITY; }
    if p >= 1.0 { return f64::INFINITY; }
    let plow = 0.02425;
    let phigh = 1.0 - 0.02425;
    if p < plow {
        let q = (-2.0 * (1.0 - p).ln()).sqrt();
        ppf_low(q)
    } else if p <= phigh {
        let q = p - 0.5;
        ppf_mid(q)
    } else {
        let q = (-2.0 * p.ln()).sqrt();
        -ppf_low(q)
    }
}

// Wilson score interval for a proportion (k successes out of n).
pub fn wilson_ci(k: f64, n: f64, z: f64) -> [f64; 2] {
    if n <= 0.0 { return [0.0, 1.0]; }
    let p = k / n;
    let denom = 1.0 + z * z / n;
    let center = (p + z * z / (2.0 * n)) / denom;
    let margin = (z * ((p * (1.0 - p) / n) + z * z / (4.0 * n * n)).sqrt()) / denom;
    [(center - margin).max(0.0), (center + margin).min(1.0)]
}

// Seeded xorshift RNG -> uniform [0,1).
pub fn make_rng(seed: u64) -> impl FnMut() -> f64 {
    let mut s = if seed == 0 { 0x9E37_9B97_9F4A_7C15 } else { seed };
    move || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        ((s >> 11) as f64) / ((1u64 << 53) as f64)
    }
}

// Standard-normal sample via Box-Muller.
pub fn normal_sample(rng: &mut impl FnMut() -> f64) -> f64 {
    let u1 = rng();
    let u2 = rng();
    // Clamp u1 into (0,1) so ln() is finite, but keep its value (don't push it
    // toward 1 — that collapses the Gaussian magnitude to ~0).
    let r = u1.clamp(1e-12, 1.0 - 1e-12);
    (-2.0_f64 * r.ln()).sqrt() * (2.0_f64 * std::f64::consts::PI * u2).cos()
}

// Percentile of an already-sorted slice.
pub fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() { return 0.0; }
    let idx = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}
