//! Small numeric helpers: normal CDF, quantiles, GBM barrier-hit probability.

/// Standard normal CDF via erfc (W. J. Cody-style rational approximation, |err| < 1.2e-7).
pub fn norm_cdf(x: f64) -> f64 {
    0.5 * erfc(-x / std::f64::consts::SQRT_2)
}

fn erfc(x: f64) -> f64 {
    let z = x.abs();
    let t = 1.0 / (1.0 + 0.5 * z);
    let r = t * (-z * z - 1.26551223
        + t * (1.00002368
            + t * (0.37409196
                + t * (0.09678418
                    + t * (-0.18628806
                        + t * (0.27886807
                            + t * (-1.13520398 + t * (1.48851587 + t * (-0.82215223 + t * 0.17087277)))))))))
        .exp();
    if x >= 0.0 { r } else { 2.0 - r }
}

/// Inverse standard normal CDF (Acklam's algorithm, rel. err ~1e-9).
pub fn norm_inv(p: f64) -> f64 {
    const A: [f64; 6] = [-3.969683028665376e1, 2.209460984245205e2, -2.759285104469687e2, 1.38357751867269e2, -3.066479806614716e1, 2.506628277459239];
    const B: [f64; 5] = [-5.447609879822406e1, 1.615858368580409e2, -1.556989798598866e2, 6.680131188771972e1, -1.328068155288572e1];
    const C: [f64; 6] = [-7.784894002430293e-3, -3.223964580411365e-1, -2.400758277161838, -2.549732539343734, 4.374664141464968, 2.938163982698783];
    const D: [f64; 4] = [7.784695709041462e-3, 3.224671290700398e-1, 2.445134137142996, 3.754408661907416];
    let p_low = 0.02425;
    if p < p_low {
        let q = (-2.0 * p.ln()).sqrt();
        (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5]) / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    } else if p <= 1.0 - p_low {
        let q = p - 0.5;
        let r = q * q;
        (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q
            / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
    } else {
        -norm_inv(1.0 - p)
    }
}

/// Linear-interpolated quantile of an already sorted slice.
pub fn quantile_sorted(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let pos = q.clamp(0.0, 1.0) * (sorted.len() - 1) as f64;
    let lo = pos.floor() as usize;
    let hi = pos.ceil() as usize;
    sorted[lo] + (sorted[hi] - sorted[lo]) * (pos - lo as f64)
}

/// Geometric Brownian motion parameters for a rate, annualised, as fractions.
#[derive(Debug, Clone, Copy)]
pub struct Gbm {
    pub drift: f64,
    pub vol: f64,
}

impl Gbm {
    /// Probability that the running maximum of S reaches `barrier` within `years`,
    /// starting from `spot`. Reflection-principle closed form for drifted Brownian motion
    /// in log space. Barrier at or below spot is a certain hit.
    pub fn prob_hit_above(&self, spot: f64, barrier: f64, years: f64) -> f64 {
        if barrier <= spot {
            return 1.0;
        }
        if years <= 0.0 || self.vol <= 0.0 {
            let nu = self.drift - 0.5 * self.vol * self.vol;
            return if nu * years >= (barrier / spot).ln() { 1.0 } else { 0.0 };
        }
        let b = (barrier / spot).ln();
        let nu = self.drift - 0.5 * self.vol * self.vol;
        let s = self.vol * years.sqrt();
        let below = norm_cdf((b - nu * years) / s)
            - (2.0 * nu * b / (self.vol * self.vol)).exp() * norm_cdf((-b - nu * years) / s);
        (1.0 - below).clamp(0.0, 1.0)
    }

    /// Quantile of S at `years` given spot.
    pub fn quantile(&self, spot: f64, years: f64, q: f64) -> f64 {
        let nu = self.drift - 0.5 * self.vol * self.vol;
        spot * (nu * years + self.vol * years.sqrt() * norm_inv(q)).exp()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn norm_cdf_known_values() {
        assert_relative_eq!(norm_cdf(0.0), 0.5, epsilon = 1e-7);
        assert_relative_eq!(norm_cdf(1.96), 0.9750021, epsilon = 1e-6);
        assert_relative_eq!(norm_cdf(-1.0), 0.1586553, epsilon = 1e-6);
    }

    #[test]
    fn norm_inv_round_trips() {
        for p in [0.01, 0.1, 0.5, 0.9, 0.99] {
            assert_relative_eq!(norm_cdf(norm_inv(p)), p, epsilon = 1e-6);
        }
    }

    #[test]
    fn hit_probability_without_drift_is_twice_tail_probability() {
        // With nu = 0 the reflection principle gives P(max >= b) = 2 * P(X_T >= b).
        let vol = 0.1;
        let g = Gbm { drift: 0.5 * vol * vol, vol };
        let p = g.prob_hit_above(1.0, 1.05, 0.25);
        let expected = 2.0 * (1.0 - norm_cdf(1.05f64.ln() / (vol * 0.5)));
        assert_relative_eq!(p, expected, epsilon = 1e-9);
    }

    #[test]
    fn hit_probability_is_monotone_in_time() {
        let g = Gbm { drift: 0.0, vol: 0.08 };
        let a = g.prob_hit_above(3.89, 3.93, 30.0 / 365.0);
        let b = g.prob_hit_above(3.89, 3.93, 90.0 / 365.0);
        assert!(a < b && b < 1.0 && a > 0.0);
    }
}
