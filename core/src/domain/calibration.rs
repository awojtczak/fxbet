//! How well-calibrated were your probabilities, compared with the model's?

use serde::Serialize;

#[derive(Debug, Clone)]
pub struct ResolvedBet {
    pub subjective_prob: Option<f64>,
    pub model_prob: f64,
    pub target_hit: bool,
    pub edge_vs_day0_pln: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Calibration {
    pub resolved: usize,
    pub hits: usize,
    pub brier_subjective: Option<f64>,
    pub brier_model: Option<f64>,
    /// Brier on the same subset of bets where you gave a probability — the fair comparison.
    pub brier_model_on_subjective_subset: Option<f64>,
    pub total_edge_vs_day0_pln: f64,
    pub buckets_subjective: Vec<Bucket>,
    pub buckets_model: Vec<Bucket>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Bucket {
    pub from: f64,
    pub to: f64,
    pub count: usize,
    pub mean_predicted: f64,
    pub hit_rate: f64,
}

pub fn calibrate(bets: &[ResolvedBet]) -> Calibration {
    let outcome = |b: &ResolvedBet| if b.target_hit { 1.0 } else { 0.0 };
    let with_subj: Vec<(f64, f64, f64)> =
        bets.iter().filter_map(|b| b.subjective_prob.map(|p| (p, b.model_prob, outcome(b)))).collect();
    let mean = |xs: &mut dyn Iterator<Item = f64>| {
        let (sum, n) = xs.fold((0.0, 0usize), |(s, n), x| (s + x, n + 1));
        (n > 0).then(|| sum / n as f64)
    };
    Calibration {
        resolved: bets.len(),
        hits: bets.iter().filter(|b| b.target_hit).count(),
        brier_subjective: mean(&mut with_subj.iter().map(|(p, _, o)| (p - o).powi(2))),
        brier_model: mean(&mut bets.iter().map(|b| (b.model_prob - outcome(b)).powi(2))),
        brier_model_on_subjective_subset: mean(&mut with_subj.iter().map(|(_, m, o)| (m - o).powi(2))),
        total_edge_vs_day0_pln: bets.iter().map(|b| b.edge_vs_day0_pln).sum(),
        buckets_subjective: buckets(with_subj.iter().map(|(p, _, o)| (*p, *o))),
        buckets_model: buckets(bets.iter().map(|b| (b.model_prob, outcome(b)))),
    }
}

fn buckets(points: impl Iterator<Item = (f64, f64)>) -> Vec<Bucket> {
    const N: usize = 5;
    let mut acc = [(0usize, 0.0f64, 0.0f64); N];
    for (p, o) in points {
        let i = ((p * N as f64) as usize).min(N - 1);
        acc[i].0 += 1;
        acc[i].1 += p;
        acc[i].2 += o;
    }
    acc.iter()
        .enumerate()
        .filter(|(_, (c, _, _))| *c > 0)
        .map(|(i, (c, p, o))| Bucket {
            from: i as f64 / N as f64,
            to: (i + 1) as f64 / N as f64,
            count: *c,
            mean_predicted: p / *c as f64,
            hit_rate: o / *c as f64,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn brier_scores_compare_subjective_and_model() {
        let bets = [
            ResolvedBet { subjective_prob: Some(0.9), model_prob: 0.5, target_hit: true, edge_vs_day0_pln: 100.0 },
            ResolvedBet { subjective_prob: Some(0.2), model_prob: 0.5, target_hit: false, edge_vs_day0_pln: -20.0 },
            ResolvedBet { subjective_prob: None, model_prob: 0.3, target_hit: false, edge_vs_day0_pln: 5.0 },
        ];
        let c = calibrate(&bets);
        assert_relative_eq!(c.brier_subjective.unwrap(), (0.01 + 0.04) / 2.0, epsilon = 1e-12);
        assert_relative_eq!(c.brier_model_on_subjective_subset.unwrap(), 0.25, epsilon = 1e-12);
        assert_relative_eq!(c.total_edge_vs_day0_pln, 85.0, epsilon = 1e-12);
        assert_eq!(c.buckets_subjective.len(), 2);
    }
}
