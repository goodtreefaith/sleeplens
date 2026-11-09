//! Scoring for stage estimates against polysomnography labels.
//!
//! The evidence review is blunt about what makes a staging claim meaningful:
//! accuracy alone hides class failure, and a single pooled confusion matrix
//! cannot show whether one participant carried the result. So every metric here
//! is reported per class, at three label granularities, with a
//! participant-level bootstrap interval, and next to two trivial baselines.
//!
//! # How abstention is scored
//!
//! SleepLens emits `Uncertain` for epochs its confidence model cannot support.
//! There are two honest ways to report that, and they answer different
//! questions:
//!
//! * **Coverage view** (the default): uncertain epochs are excluded from the
//!   confusion matrix, and coverage is reported alongside. This says "of what it
//!   was willing to score, how often was it right" and cannot be inflated by
//!   abstaining on everything only if coverage is read with it.
//! * **Closed-set view**: uncertain epochs are forced to `Light`, the harmless
//!   default, and every epoch counts. This says "how right is the output a user
//!   would actually see".
//!
//! Both are computed and reported together. Reporting either alone would be
//! misleading.

use crate::night::StageLabel;
use std::collections::BTreeMap;

/// The four classes SleepLens can emit, in a fixed order used by every matrix.
pub const CLASSES: [StageLabel; 4] = [
    StageLabel::Wake,
    StageLabel::Light,
    StageLabel::Deep,
    StageLabel::Rem,
];

pub fn class_index(stage: StageLabel) -> Option<usize> {
    CLASSES.iter().position(|c| *c == stage)
}

pub fn class_name(stage: StageLabel) -> &'static str {
    match stage {
        StageLabel::Wake => "Wake",
        StageLabel::Light => "Light",
        StageLabel::Deep => "Deep",
        StageLabel::Rem => "REM",
        StageLabel::Excluded => "Excluded",
    }
}

/// Cohen's kappa for an arbitrary confusion matrix.
///
/// `matrix[truth][predicted]`. Returns zero when chance agreement is total, so a
/// degenerate classifier is not rewarded with a division by zero.
pub fn kappa(matrix: &[[usize; 4]; 4]) -> f64 {
    let total: usize = matrix.iter().flatten().sum();
    if total == 0 {
        return 0.0;
    }
    let mut observed = 0usize;
    let mut expected = 0.0;
    for i in 0..4 {
        observed += matrix[i][i];
        let row: usize = matrix[i].iter().sum();
        let column: usize = (0..4).map(|j| matrix[j][i]).sum();
        expected += row as f64 * column as f64;
    }
    let expected = expected / (total * total) as f64;
    let observed = observed as f64 / total as f64;
    if (1.0 - expected).abs() < 1e-12 {
        return 0.0;
    }
    (observed - expected) / (1.0 - expected)
}

/// Per-class precision, recall and F1.
#[derive(Clone, Copy, Debug, Default)]
pub struct ClassScore {
    pub truth: usize,
    pub predicted: usize,
    pub correct: usize,
    pub precision: f64,
    pub recall: f64,
    pub f1: f64,
}

impl ClassScore {
    fn from_matrix(matrix: &[[usize; 4]; 4], class: usize) -> Self {
        let truth: usize = matrix[class].iter().sum();
        let predicted: usize = (0..4).map(|i| matrix[i][class]).sum();
        let correct = matrix[class][class];
        let precision = if predicted == 0 {
            // A class never predicted has undefined precision. Reporting zero
            // keeps macro averages honest instead of silently skipping it.
            0.0
        } else {
            correct as f64 / predicted as f64
        };
        let recall = if truth == 0 {
            0.0
        } else {
            correct as f64 / truth as f64
        };
        let f1 = if precision + recall == 0.0 {
            0.0
        } else {
            2.0 * precision * recall / (precision + recall)
        };
        Self {
            truth,
            predicted,
            correct,
            precision,
            recall,
            f1,
        }
    }
}

/// Everything one evaluation run reports for one label granularity.
#[derive(Clone, Debug)]
pub struct Report {
    pub name: &'static str,
    pub matrix: [[usize; 4]; 4],
    pub epochs: usize,
    pub accuracy: f64,
    pub kappa: f64,
    pub classes: [ClassScore; 4],
    /// Macro F1 over classes that appear in the labels at all, so a cohort
    /// without N3 does not get penalised for never predicting it.
    pub macro_f1: f64,
}

impl Report {
    pub fn from_matrix(name: &'static str, matrix: [[usize; 4]; 4]) -> Self {
        let epochs: usize = matrix.iter().flatten().sum();
        let correct: usize = (0..4).map(|i| matrix[i][i]).sum();
        let classes = std::array::from_fn(|c| ClassScore::from_matrix(&matrix, c));
        let present: Vec<f64> = (0..4)
            .filter(|&c| classes[c].truth > 0)
            .map(|c| classes[c].f1)
            .collect();
        Report {
            name,
            matrix,
            epochs,
            accuracy: if epochs == 0 {
                0.0
            } else {
                correct as f64 / epochs as f64
            },
            kappa: kappa(&matrix),
            classes,
            macro_f1: if present.is_empty() {
                0.0
            } else {
                present.iter().sum::<f64>() / present.len() as f64
            },
        }
    }

    /// Collapse to Wake / NREM / REM. Deep merges into NREM; Light is NREM.
    pub fn three_class(matrix: &[[usize; 4]; 4]) -> [[usize; 4]; 4] {
        // Target order stays the same array shape, but only indices 0, 1 and 3
        // carry meaning: Wake, NREM, REM. Deep predictions and truths fold in.
        let mut out = [[0usize; 4]; 4];
        let map = |i: usize| -> usize {
            match CLASSES[i] {
                StageLabel::Wake => 0,
                StageLabel::Light | StageLabel::Deep => 1,
                StageLabel::Rem => 3,
                StageLabel::Excluded => 4,
            }
        };
        for truth in 0..4 {
            for predicted in 0..4 {
                out[map(truth)][map(predicted)] += matrix[truth][predicted];
            }
        }
        out
    }

    /// Collapse to sleep versus wake.
    pub fn sleep_wake(matrix: &[[usize; 4]; 4]) -> [[usize; 4]; 4] {
        let mut out = [[0usize; 4]; 4];
        for truth in 0..4 {
            for predicted in 0..4 {
                let t = usize::from(CLASSES[truth] != StageLabel::Wake);
                let p = usize::from(CLASSES[predicted] != StageLabel::Wake);
                out[t][p] += matrix[truth][predicted];
            }
        }
        out
    }

    /// Human-readable table, safe to print to a terminal or a file.
    pub fn render(&self) -> String {
        use std::fmt::Write;
        let mut out = String::new();
        let _ = writeln!(
            out,
            "{}: {} epochs, accuracy {:.1}%, kappa {:.3}, macro F1 {:.3}",
            self.name,
            self.epochs,
            self.accuracy * 100.0,
            self.kappa,
            self.macro_f1
        );
        let _ = writeln!(
            out,
            "  {:<8} {:>8} {:>9} {:>9} {:>7}",
            "class", "epochs", "recall", "precision", "F1"
        );
        for (i, class) in self.classes.iter().enumerate() {
            if class.truth == 0 && class.predicted == 0 {
                continue;
            }
            let _ = writeln!(
                out,
                "  {:<8} {:>8} {:>8.1}% {:>8.1}% {:>7.3}",
                class_name(CLASSES[i]),
                class.truth,
                class.recall * 100.0,
                class.precision * 100.0,
                class.f1
            );
        }
        out
    }
}

/// One night's contribution, kept so the bootstrap can resample participants.
#[derive(Clone, Debug)]
pub struct NightOutcome {
    pub participant: String,
    /// Truth/prediction pairs for every epoch that had both a label and output.
    pub pairs: Vec<(StageLabel, StageLabel)>,
    /// Epochs the estimate refused to score.
    pub abstained: usize,
    /// Epochs with a label and usable flow.
    pub scored: usize,
}

impl NightOutcome {
    pub fn accuracy(&self) -> f64 {
        if self.pairs.is_empty() {
            return 0.0;
        }
        let correct = self.pairs.iter().filter(|(t, p)| t == p).count();
        correct as f64 / self.pairs.len() as f64
    }

    pub fn coverage(&self) -> f64 {
        let total = self.scored + self.abstained;
        if total == 0 {
            0.0
        } else {
            self.scored as f64 / total as f64
        }
    }
}

/// Accumulate a confusion matrix from labelled/estimated pairs.
#[derive(Clone, Debug, Default)]
pub struct Tally {
    pub matrix: [[usize; 4]; 4],
    pub abstained: usize,
    pub pairs: Vec<(StageLabel, StageLabel)>,
    pub per_night: Vec<NightOutcome>,
}

impl Tally {
    /// Add one night. `estimated` runs parallel to `labels`.
    ///
    /// * `None` — no estimate at all for that epoch (mask off, no signal).
    /// * `Some(None)` — the estimate abstained (`Uncertain`).
    /// * `Some(Some(stage))` — a claimed stage.
    ///
    /// Excluded labels are skipped. With `force_abstentions` an abstention is
    /// scored as Light, producing the closed-set view; without it abstentions
    /// are counted but kept out of the matrix, producing the coverage view.
    pub fn add_night(
        &mut self,
        participant: &str,
        labels: &[StageLabel],
        estimated: &[Option<Option<StageLabel>>],
        force_abstentions: bool,
    ) {
        let mut outcome = NightOutcome {
            participant: participant.to_string(),
            pairs: Vec::new(),
            abstained: 0,
            scored: 0,
        };
        for (epoch, label) in labels.iter().enumerate() {
            if *label == StageLabel::Excluded {
                continue;
            }
            // Outer `None` is a missing estimate; inner `None` is an abstention.
            let Some(entry) = estimated.get(epoch) else {
                continue;
            };
            let Some(entry) = entry else {
                continue;
            };
            let predicted = entry.unwrap_or(StageLabel::Light);
            if entry.is_none() {
                outcome.abstained += 1;
                if !force_abstentions {
                    continue;
                }
            }
            let (Some(t), Some(p)) = (class_index(*label), class_index(predicted)) else {
                continue;
            };
            self.matrix[t][p] += 1;
            outcome.pairs.push((*label, predicted));
            outcome.scored += 1;
        }
        self.abstained += outcome.abstained;
        self.per_night.push(outcome);
    }

    pub fn report(&self, name: &'static str) -> Report {
        Report::from_matrix(name, self.matrix)
    }

    /// Mean of per-night accuracy, which weights every night equally instead of
    /// letting the longest recording dominate.
    pub fn mean_night_accuracy(&self) -> f64 {
        if self.per_night.is_empty() {
            return 0.0;
        }
        self.per_night
            .iter()
            .map(NightOutcome::accuracy)
            .sum::<f64>()
            / self.per_night.len() as f64
    }
}

/// Participant-level bootstrap interval for any per-night statistic.
///
/// Resampling participants, not epochs, is the point: epochs within a night are
/// strongly dependent, so an epoch-level interval would be far too narrow. This
/// is the correction the evidence review asks for.
pub struct Bootstrap<'a> {
    pub nights: &'a [&'a NightOutcome],
}

impl<'a> Bootstrap<'a> {
    /// Percentile interval for a statistic computed over a set of nights.
    /// `iterations` of 2000 is plenty for two decimal places.
    pub fn interval(
        &self,
        statistic: impl Fn(&[&NightOutcome]) -> f64,
        iterations: usize,
        seed: u64,
    ) -> Option<(f64, f64)> {
        if self.nights.len() < 2 {
            return None;
        }
        // Group by participant so repeat nights stay together.
        let mut by_participant: BTreeMap<&str, Vec<&NightOutcome>> = BTreeMap::new();
        for night in self.nights {
            by_participant
                .entry(night.participant.as_str())
                .or_default()
                .push(night);
        }
        let participants: Vec<Vec<&NightOutcome>> = by_participant.into_values().collect();
        if participants.len() < 2 {
            return None;
        }
        let mut state = seed | 1;
        let mut next = move || {
            // xorshift64*: adequate for resampling indices.
            state ^= state >> 12;
            state ^= state << 25;
            state ^= state >> 27;
            state.wrapping_mul(0x2545_F491_4F6C_DD1D)
        };
        let mut values = Vec::with_capacity(iterations);
        for _ in 0..iterations {
            let mut sample: Vec<&NightOutcome> = Vec::new();
            for _ in 0..participants.len() {
                let pick = (next() % participants.len() as u64) as usize;
                sample.extend(participants[pick].iter().copied());
            }
            values.push(statistic(&sample));
        }
        values.sort_by(f64::total_cmp);
        let low = values[(values.len() as f64 * 0.025) as usize];
        let high = values[((values.len() as f64 * 0.975) as usize).min(values.len() - 1)];
        Some((low, high))
    }
}

/// Kappa of a set of nights, recomputing the matrix from scratch.
pub fn kappa_of(nights: &[&NightOutcome]) -> f64 {
    let mut matrix = [[0usize; 4]; 4];
    for night in nights {
        for (truth, predicted) in &night.pairs {
            if let (Some(t), Some(p)) = (class_index(*truth), class_index(*predicted)) {
                matrix[t][p] += 1;
            }
        }
    }
    kappa(&matrix)
}

/// Accuracy of a set of nights.
pub fn accuracy_of(nights: &[&NightOutcome]) -> f64 {
    let total: usize = nights.iter().map(|n| n.pairs.len()).sum();
    if total == 0 {
        return 0.0;
    }
    let correct: usize = nights
        .iter()
        .flat_map(|n| &n.pairs)
        .filter(|(t, p)| t == p)
        .count();
    correct as f64 / total as f64
}

/// Trivial reference points. A model that cannot beat these is not a model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Baseline {
    /// Predict Light everywhere.
    AlwaysLight,
    /// Predict the most common class in the labels, which is nearly always Light.
    Majority,
    /// Predict Wake for the first quarter of the night, then Light.
    TimeOnly,
}

impl Baseline {
    pub fn name(self) -> &'static str {
        match self {
            Baseline::AlwaysLight => "baseline: always Light",
            Baseline::Majority => "baseline: majority class",
            Baseline::TimeOnly => "baseline: time of night only",
        }
    }

    pub fn predict(self, epoch: usize, epochs: usize) -> StageLabel {
        match self {
            Baseline::AlwaysLight | Baseline::Majority => StageLabel::Light,
            Baseline::TimeOnly => {
                if epoch * 4 < epochs {
                    StageLabel::Wake
                } else {
                    StageLabel::Light
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kappa_matches_hand_computation() {
        // 50 agreements of 100 with balanced 50/50 classes gives chance 0.5.
        let mut m = [[0usize; 4]; 4];
        m[0][0] = 25;
        m[1][1] = 25;
        m[0][1] = 25;
        m[1][0] = 25;
        // observed 0.5, chance 0.5 -> kappa 0
        assert!((kappa(&m) - 0.0).abs() < 1e-9);
        let mut m = [[0usize; 4]; 4];
        m[0][0] = 50;
        m[1][1] = 50;
        assert!((kappa(&m) - 1.0).abs() < 1e-9);
        assert_eq!(kappa(&[[0; 4]; 4]), 0.0);
    }

    #[test]
    fn per_class_scores_of_known_matrix() {
        let mut m = [[0usize; 4]; 4];
        m[0][0] = 8;
        m[0][1] = 2;
        m[1][1] = 90;
        let report = Report::from_matrix("test", m);
        assert_eq!(report.epochs, 100);
        assert!((report.accuracy - 0.98).abs() < 1e-9);
        assert!((report.classes[0].recall - 0.8).abs() < 1e-9);
        assert!((report.classes[0].precision - 1.0).abs() < 1e-9);
        assert!((report.classes[3].precision - 0.0).abs() < 1e-9);
    }

    #[test]
    fn collapsing_to_three_classes_and_sleep_wake_preserves_counts() {
        let mut m = [[0usize; 4]; 4];
        m[0][0] = 10;
        m[2][2] = 30;
        m[3][3] = 20;
        m[1][3] = 5;
        let three = Report::three_class(&m);
        // Light and Deep both collapse to NREM.
        assert_eq!(three[1][1], 30);
        assert_eq!(three[0][0], 10);
        assert_eq!(three[3][3], 20);
        // A Light truth predicted REM lands on NREM-vs-REM.
        assert_eq!(three[1][3], 5);
        assert_eq!(three.iter().flatten().sum::<usize>(), 65);
        let binary = Report::sleep_wake(&m);
        assert_eq!(binary[0][0], 10);
        // Light-as-REM is still sleep-as-sleep once collapsed.
        assert_eq!(binary[1][1], 55);
        // Nothing is ever predicted awake while the truth is sleep here.
        assert_eq!(binary[1][0], 0);
    }

    #[test]
    fn night_outcomes_exclude_gaps_and_record_abstentions() {
        let mut tally = Tally::default();
        let labels = [
            StageLabel::Wake,
            StageLabel::Light,
            StageLabel::Excluded,
            StageLabel::Deep,
        ];
        let estimate = [
            Some(Some(StageLabel::Wake)),
            None,
            Some(Some(StageLabel::Light)),
            Some(None),
        ];
        tally.add_night("p1", &labels, &estimate, false);
        // Only the first epoch is both labelled and claimed.
        assert_eq!(tally.matrix.iter().flatten().sum::<usize>(), 1);
        assert_eq!(tally.per_night[0].pairs.len(), 1);
        // The abstention is recorded but not scored in the coverage view.
        assert_eq!(tally.abstained, 1);

        let mut closed = Tally::default();
        closed.add_night("p1", &labels, &estimate, true);
        // Closed set also scores the abstention as Light, which is wrong here:
        // truth Deep (index 2) against a Light claim.
        assert_eq!(closed.matrix.iter().flatten().sum::<usize>(), 2);
        assert_eq!(closed.matrix[2][1], 1);
    }

    #[test]
    fn macro_f1_ignores_classes_absent_from_the_labels() {
        let mut m = [[0usize; 4]; 4];
        m[1][1] = 100;
        let report = Report::from_matrix("test", m);
        assert!((report.macro_f1 - 1.0).abs() < 1e-9);
    }

    #[test]
    fn bootstrap_is_deterministic_and_brackets_the_estimate() {
        let nights: Vec<NightOutcome> = (0..8)
            .map(|i| NightOutcome {
                participant: format!("p{}", i % 4),
                pairs: vec![(StageLabel::Light, StageLabel::Light); 10 + i],
                abstained: 0,
                scored: 10 + i,
            })
            .collect();
        let refs: Vec<&NightOutcome> = nights.iter().collect();
        let bootstrap = Bootstrap { nights: &refs };
        let a = bootstrap.interval(accuracy_of, 200, 42).unwrap();
        let b = bootstrap.interval(accuracy_of, 200, 42).unwrap();
        assert_eq!(a, b);
        assert!(a.0 <= a.1);
        assert!(a.0 <= 1.0 && a.1 <= 1.0);
        let single_nights = [&nights[0]];
        let single = Bootstrap {
            nights: &single_nights,
        };
        assert!(single.interval(accuracy_of, 100, 1).is_none());
    }

    #[test]
    fn baselines_are_what_they_say() {
        assert_eq!(Baseline::AlwaysLight.predict(0, 100), StageLabel::Light);
        assert_eq!(Baseline::TimeOnly.predict(0, 100), StageLabel::Wake);
        assert_eq!(Baseline::TimeOnly.predict(30, 100), StageLabel::Light);
    }
}
