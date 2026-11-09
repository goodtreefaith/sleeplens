//! Run the SleepLens estimator over labelled nights and score it.
//!
//! This is the piece that makes accuracy a measurement instead of a claim.
//! It reads a directory of paired recordings and annotations, converts each
//! night to estimator input, scores it, and reports every metric the evidence
//! review asks for:
//!
//! * overall accuracy and Cohen's kappa at four, three and two classes;
//! * per-class recall and precision, because a good kappa can hide total
//!   failure on Deep or REM;
//! * participant-level bootstrap intervals, so a result cannot be carried by
//!   one unusually easy recording;
//! * coverage, so abstaining on hard epochs cannot inflate accuracy;
//! * the trivial baselines that any real model has to beat;
//! * per-participant outcomes, for inspecting the failure modes.

use std::fmt::Write as _;
use std::path::Path;

use sleeplens_core::sleep_stages::{SleepStage, SleepStageEstimate, SleepStageInput};

use crate::annotations;
use crate::edf::{self, Edf};
use crate::metrics::{
    accuracy_of, class_index, kappa_of, Baseline, Bootstrap, Report, Tally, CLASSES,
};
use crate::night::{labels_from_onsets, normalise_respiration, LabelledNight, StageLabel, HZ};
use crate::profusion;
use crate::rec;

/// A resolved pair of files for one night.
#[derive(Clone, Debug)]
pub struct NightFiles {
    pub id: String,
    pub signal: NightSignal,
    pub labels: NightLabels,
}

#[derive(Clone, Debug)]
pub enum NightSignal {
    /// A header-and-records file: EDF, EDF+, or the ucddb `.rec` variant.
    Edf(std::path::PathBuf),
}

#[derive(Clone, Debug)]
pub enum NightLabels {
    /// A separate EDF hypnogram file (Sleep-EDF pairs).
    EdfHypnogram(std::path::PathBuf),
    /// Compumedics Profusion XML (NSRR MESA/SHHS).
    ProfusionXml(std::path::PathBuf),
    /// One stage code per 30-second epoch, in a plain text file (ucddb).
    StageCodes(std::path::PathBuf),
}

/// How a directory of PSG data is laid out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    /// `SC4001E0-PSG.edf` with `SC4001EC-Hypnogram.edf` (PhysioNet Sleep-EDF).
    /// Respiration is 1 Hz here, so breath-level features cannot be evaluated
    /// from it; it is kept for sleep/wake comparison only.
    SleepEdf,
    /// `mesa-sleep-0001.edf` with `mesa-sleep-0001-profusion.xml` (NSRR).
    Nsrr,
    /// `ucddb002.rec` with `ucddb002_stage.txt` (PhysioNet ucddb).
    Ucddb,
}

impl Layout {
    pub fn parse(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "sleep-edf" | "sleepedf" | "edfx" => Some(Layout::SleepEdf),
            "nsrr" | "mesa" | "shhs" => Some(Layout::Nsrr),
            "ucddb" => Some(Layout::Ucddb),
            _ => None,
        }
    }
}

/// Find every night in `root`, pairing signal and label files.
///
/// Pairing is by file stem with the format's suffixes stripped, so it works for
/// both known layouts without a manifest. Unpaired files are reported, never
/// guessed at.
pub fn discover(root: &Path, layout: Layout) -> Result<(Vec<NightFiles>, Vec<String>), String> {
    if !root.is_dir() {
        return Err(format!("{} is not a directory", root.display()));
    }
    let mut edfs = Vec::new();
    let mut xmls = Vec::new();
    let mut recs = Vec::new();
    let mut texts = Vec::new();
    collect(root, &mut edfs, &mut xmls, &mut recs, &mut texts);
    edfs.sort();
    xmls.sort();
    recs.sort();
    texts.sort();

    let mut nights = Vec::new();
    let mut unpaired = Vec::new();
    match layout {
        Layout::SleepEdf => {
            for path in &edfs {
                let name = file_stem(path);
                let Some(recording) = name.strip_suffix("-PSG") else {
                    continue;
                };
                // The final recording letter is E, F or G; the hypnogram's
                // last character identifies its scorer, not its participant.
                let bytes = recording.as_bytes();
                if bytes.len() != 8
                    || !recording.starts_with("SC4")
                    || !bytes[3..6].iter().all(u8::is_ascii_digit)
                    || !bytes[6].is_ascii_uppercase()
                    || bytes[7] != b'0'
                {
                    continue;
                }
                let prefix = &recording[..7];
                let hypnogram = edfs.iter().find(|candidate| {
                    let stem = file_stem(candidate);
                    stem.strip_suffix("-Hypnogram").is_some_and(|head| {
                        head.len() == 8
                            && head.starts_with(prefix)
                            && head.as_bytes()[7].is_ascii_uppercase()
                    })
                });
                match hypnogram {
                    Some(hypnogram) => nights.push(NightFiles {
                        id: name.clone(),
                        signal: NightSignal::Edf(path.clone()),
                        labels: NightLabels::EdfHypnogram(hypnogram.clone()),
                    }),
                    None => unpaired.push(format!("{name}: no hypnogram")),
                }
            }
        }
        Layout::Nsrr => {
            for path in &edfs {
                let name = file_stem(path);
                let annotation = xmls.iter().find(|candidate| {
                    let stem = file_stem(candidate);
                    stem.strip_suffix("-profusion").unwrap_or(&stem) == name
                });
                match annotation {
                    Some(annotation) => nights.push(NightFiles {
                        id: name.clone(),
                        signal: NightSignal::Edf(path.clone()),
                        labels: NightLabels::ProfusionXml(annotation.clone()),
                    }),
                    None => unpaired.push(format!("{name}: no profusion annotation")),
                }
            }
        }
        Layout::Ucddb => {
            for path in &recs {
                let name = file_stem(path);
                let stage = texts
                    .iter()
                    .find(|candidate| file_stem(candidate) == format!("{name}_stage"));
                match stage {
                    Some(stage) => nights.push(NightFiles {
                        id: name.clone(),
                        signal: NightSignal::Edf(path.clone()),
                        labels: NightLabels::StageCodes(stage.clone()),
                    }),
                    None => unpaired.push(format!("{name}: no stage annotation")),
                }
            }
        }
    }
    Ok((nights, unpaired))
}

fn collect(
    dir: &Path,
    edfs: &mut Vec<std::path::PathBuf>,
    xmls: &mut Vec<std::path::PathBuf>,
    recs: &mut Vec<std::path::PathBuf>,
    texts: &mut Vec<std::path::PathBuf>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, edfs, xmls, recs, texts);
            continue;
        }
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        if name.ends_with(".edf") {
            edfs.push(path);
        } else if name.ends_with(".xml") {
            xmls.push(path);
        } else if name.ends_with(".rec") {
            recs.push(path);
        } else if name.ends_with("_stage.txt") {
            texts.push(path);
        }
    }
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Participant identifier from a recording name, used to keep repeat nights
/// together in the bootstrap.
///
/// Sleep-EDF uses `SC4ssNE0`: `ss` is the person and `N` the night, so
/// `SC4001E0` and `SC4002E0` belong to the same participant. NSRR names carry a sequence number, which is
/// still stable per participant.
pub fn participant_of(name: &str) -> String {
    let bytes = name.as_bytes();
    if bytes.len() >= 8
        && (name.starts_with("SC4") || name.starts_with("ST7"))
        && bytes[3..6].iter().all(u8::is_ascii_digit)
    {
        return name[..5].to_string();
    }
    if let Some(rest) = name.trim_start_matches("mesa-sleep-").strip_prefix("shhs") {
        // `shhs1-200001` -> the participant is the numeric component.
        return rest
            .split(['-', '_'])
            .find(|part| part.len() > 2 && part.chars().all(|c| c.is_ascii_digit()))
            .unwrap_or(rest)
            .to_string();
    }
    let base = name.trim_start_matches("mesa-sleep-");
    let base = base.strip_suffix("-PSG").unwrap_or(base);
    // Drop a trailing visit/night code such as E0, E1, EC or EH.
    let trimmed = base
        .strip_suffix("E0")
        .or_else(|| base.strip_suffix("E1"))
        .or_else(|| base.strip_suffix("-profusion"))
        .unwrap_or(base);
    trimmed
        .split(['-', '_'])
        .next()
        .unwrap_or(trimmed)
        .to_string()
}

/// Load a night: normalised respiration at 50 Hz plus one label per epoch.
pub fn load(night: &NightFiles) -> Result<LabelledNight, String> {
    load_with_channel(night, None)
}

/// Explicit channel selection for research cohorts, with no silent fallback.
pub fn load_with_channel(
    night: &NightFiles,
    preferred: Option<&str>,
) -> Result<LabelledNight, String> {
    let NightSignal::Edf(signal_path) = &night.signal;
    let data = std::fs::read(signal_path).map_err(|e| format!("{}: {e}", night.id))?;
    let edf = if rec::is_rec(&data) {
        rec::parse_header(&data)
    } else {
        edf::parse_header(&data)
    }
    .map_err(|e| format!("{}: {e}", night.id))?;
    require_complete(&edf, data.len()).map_err(|e| format!("{}: {e}", night.id))?;
    let channel = preferred
        .map_or_else(
            || respiration_channel(&edf),
            |name| edf.find(&[name]).first().copied(),
        )
        .ok_or_else(|| format!("{}: no respiration channel found", night.id))?;
    let raw = edf
        .read_channel(&data, channel)
        .map_err(|e| format!("{}: {e}", night.id))?;
    let hz = edf.sample_rate(channel);
    // Thermistor channels differ in polarity. The estimator only needs a
    // consistent sign, so the default is as recorded and a documented
    // environment switch covers a channel that reads backwards.
    let invert = matches!(
        std::env::var("SLEEPLENS_INVERT_RESPIRATION").as_deref(),
        Ok("1") | Ok("true")
    );
    let flow = normalise_respiration(&raw, hz, invert);
    let epochs = flow.len().div_ceil(HZ * 30);

    let labels = match &night.labels {
        NightLabels::EdfHypnogram(path) => {
            let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", night.id))?;
            let hyp = edf::parse_header(&bytes).map_err(|e| format!("{}: {e}", night.id))?;
            require_complete(&hyp, bytes.len())
                .map_err(|e| format!("{}: hypnogram {e}", night.id))?;
            let channel = hyp
                .annotation_channel
                .ok_or_else(|| format!("{}: hypnogram is not EDF+", night.id))?;
            let list = annotations::read(&bytes, &hyp, channel)
                .map_err(|e| format!("{}: {e}", night.id))?;
            mapped_labels(&list, epochs)
        }
        NightLabels::ProfusionXml(path) => {
            let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", night.id))?;
            profusion::parse(&text).map_err(|e| format!("{}: {e}", night.id))?
        }
        NightLabels::StageCodes(path) => {
            let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", night.id))?;
            stage_codes(&text)?
        }
    };
    let mut labels = labels;
    labels.resize(epochs.max(labels.len()), StageLabel::Excluded);
    Ok(LabelledNight {
        id: night.id.clone(),
        flow,
        labels,
    })
}

fn require_complete(edf: &Edf, actual: usize) -> Result<(), String> {
    let records = if edf.is_annotation_only() {
        1
    } else {
        edf.records
    };
    let expected = records
        .checked_mul(edf.record_bytes)
        .and_then(|n| n.checked_add(edf.header_bytes))
        .ok_or("recording size overflows")?;
    if actual < expected {
        return Err(format!("truncated recording: {actual} bytes, header requires {expected}; excluded from evaluation"));
    }
    Ok(())
}

fn mapped_labels(list: &[annotations::Annotation], epochs: usize) -> Vec<StageLabel> {
    let mapped: Vec<(f64, Option<f64>, StageLabel)> = list
        .iter()
        .filter_map(|a| {
            annotations::sleep_edf_stage(&a.text)
                .map(|stage| (a.onset, a.duration, StageLabel::from_sleep_edf(stage)))
        })
        .collect();
    labels_from_onsets(&mapped, 30.0, epochs)
}

/// Parse a ucddb `*_stage.txt`: one integer per 30-second epoch.
///
/// UCDDB uses 0 Wake, 1 REM, 2 N1, 3 N2, 4 N3, 5 N4, 6 Artifact and
/// 7 Indeterminate, unlike the Profusion codes. See the dataset's annotation
/// legend at <https://physionet.org/content/ucddb/1.0.0/>.
/// Blank lines are ignored; unknown codes retain an excluded epoch.
pub fn stage_codes(text: &str) -> Result<Vec<StageLabel>, String> {
    let mut labels = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let label = match trimmed.parse::<u8>() {
            Ok(0) => StageLabel::Wake,
            Ok(1) => StageLabel::Rem,
            Ok(2 | 3) => StageLabel::Light,
            Ok(4 | 5) => StageLabel::Deep,
            _ => StageLabel::Excluded,
        };
        labels.push(label);
    }
    if labels.is_empty() {
        return Err("stage file contains no epochs".into());
    }
    Ok(labels)
}

/// Pick the respiration channel, preferring sensors that measure airflow rather
/// than effort, and falling back to effort when that is all there is.
///
/// Preference order matters: a nasal-pressure or thermistor channel is closer to
/// PAP patient flow, while belts and plethysmography measure effort and lose
/// amplitude information during obstruction.
pub fn respiration_channel(edf: &Edf) -> Option<usize> {
    let exclude = [
        "eeg", "eog", "emg", "ecg", "ekg", "temp", "marker", "annot", "spo2", "sao2",
    ];
    let usable: Vec<usize> = (0..edf.channels.len())
        .filter(|&i| {
            let label = edf.channels[i].label.to_ascii_lowercase();
            !exclude.iter().any(|x| label.contains(x))
        })
        .collect();
    let groups: [&[&str]; 4] = [
        &[
            "nasal pressure",
            "nasalpres",
            "nasal press",
            "npress",
            "cannula",
        ],
        &[
            "oro-nasal",
            "oronasal",
            "airflow",
            "thermistor",
            "therm",
            "flow",
        ],
        &["resp", "respiration"],
        &["belt", "effort", "chest", "abdomen", "thorax", "rib"],
    ];
    for patterns in groups {
        for &i in &usable {
            let label = edf.channels[i].label.to_ascii_lowercase();
            if patterns.iter().any(|p| label.contains(p)) {
                return Some(i);
            }
        }
    }
    None
}

/// Estimate one night, returning per-epoch predictions aligned to the labels.
///
/// `Some(None)` marks an abstention, `Some(Some(stage))` a claimed stage, and
/// `None` an epoch the estimator produced nothing for.
pub fn estimate(
    night: &LabelledNight,
    params: &sleeplens_core::sleep_stages::Params,
) -> Vec<Option<Option<StageLabel>>> {
    estimate_flow(&night.flow, params, &night.id)
}

/// As [`estimate`], over a raw 50 Hz flow trace.
pub fn estimate_flow(
    flow: &[f32],
    params: &sleeplens_core::sleep_stages::Params,
    start: &str,
) -> Vec<Option<Option<StageLabel>>> {
    let estimate: SleepStageEstimate = sleeplens_core::sleep_stages::estimate_sleep_stages_with(
        &SleepStageInput {
            start,
            flow,
            hz: HZ as u32,
            excess_leak: &[],
            event_offsets: &[],
        },
        params,
    );
    estimate
        .stages
        .iter()
        .map(|stage| {
            Some(match stage {
                SleepStage::Wake => Some(StageLabel::Wake),
                SleepStage::Light => Some(StageLabel::Light),
                SleepStage::Deep => Some(StageLabel::Deep),
                SleepStage::Rem => Some(StageLabel::Rem),
                SleepStage::Uncertain | SleepStage::NoData => None,
            })
        })
        .collect()
}

/// Score every night and return both views plus baselines and intervals.
pub struct Evaluation {
    pub coverage: Report,
    pub closed: Report,
    pub coverage_three: Report,
    pub closed_three: Report,
    pub coverage_binary: Report,
    pub closed_binary: Report,
    pub baselines: Vec<Report>,
    pub nights: usize,
    pub participants: usize,
    pub epochs: usize,
    pub abstained: usize,
    pub kappa_interval: Option<(f64, f64)>,
    pub accuracy_interval: Option<(f64, f64)>,
    pub coverage_mean: f64,
    pub mean_night_accuracy: f64,
    /// Per-participant accuracy, so one cohort member cannot hide failures.
    pub by_participant: Vec<(String, f64, usize)>,
}

pub fn evaluate(
    nights: &[LabelledNight],
    params: &sleeplens_core::sleep_stages::Params,
) -> Evaluation {
    let baselines = [
        Baseline::AlwaysLight,
        Baseline::Majority(cohort_majority(nights)),
        Baseline::TimeOnly,
    ];
    let mut coverage = Tally::default();
    let mut closed = Tally::default();
    let mut baseline_tallies = [Tally::default(), Tally::default(), Tally::default()];
    let mut participants = std::collections::BTreeMap::<String, (usize, usize)>::new();
    for night in nights {
        let participant = participant_of(&night.id);
        let predictions = estimate(night, params);
        coverage.add_night(&participant, &night.labels, &predictions, false);
        closed.add_night(&participant, &night.labels, &predictions, true);
        for (tally, baseline) in baseline_tallies.iter_mut().zip(baselines) {
            let forced: Vec<Option<Option<StageLabel>>> = night
                .labels
                .iter()
                .enumerate()
                .map(|(epoch, label)| {
                    (*label != StageLabel::Excluded)
                        .then(|| Some(baseline.predict(epoch, night.labels.len())))
                })
                .collect();
            tally.add_night(&participant, &night.labels, &forced, false);
        }
        if let Some(outcome) = coverage.per_night.last() {
            let entry = participants.entry(participant).or_insert((0, 0));
            entry.0 += outcome.pairs.iter().filter(|(t, p)| t == p).count();
            entry.1 += outcome.pairs.len();
        }
    }
    let refs: Vec<_> = coverage.per_night.iter().collect();
    let bootstrap = Bootstrap { nights: &refs };
    let coverage_mean = if refs.is_empty() {
        0.0
    } else {
        refs.iter().map(|n| n.coverage()).sum::<f64>() / refs.len() as f64
    };
    Evaluation {
        coverage: coverage.report("four classes, abstentions excluded"),
        closed: closed.report("four classes, abstentions scored as Light"),
        coverage_three: Report::from_matrix(
            "three classes, abstentions excluded",
            Report::three_class(&coverage.matrix),
        ),
        closed_three: Report::from_matrix(
            "three classes, abstentions scored as Light",
            Report::three_class(&closed.matrix),
        ),
        coverage_binary: Report::from_matrix(
            "sleep vs wake, abstentions excluded",
            Report::sleep_wake(&coverage.matrix),
        ),
        closed_binary: Report::from_matrix(
            "sleep vs wake, abstentions scored as Light",
            Report::sleep_wake(&closed.matrix),
        ),
        baselines: baseline_tallies
            .iter()
            .zip(baselines)
            .map(|(tally, baseline)| tally.report(baseline.name()))
            .collect(),
        nights: nights.len(),
        participants: participants.len(),
        epochs: coverage.matrix.iter().flatten().sum(),
        abstained: coverage.abstained,
        kappa_interval: bootstrap.interval(kappa_of, 2000, 0x5EED),
        accuracy_interval: bootstrap.interval(accuracy_of, 2000, 0x5EED),
        coverage_mean,
        mean_night_accuracy: coverage.mean_night_accuracy(),
        by_participant: participants
            .into_iter()
            .map(|(id, (correct, total))| {
                (
                    id,
                    if total == 0 {
                        0.0
                    } else {
                        correct as f64 / total as f64
                    },
                    total,
                )
            })
            .collect(),
    }
}

/// Ties follow CLASSES order: Wake, Light, Deep, REM. This is a descriptive
/// reference derived from the evaluated labels, not a trained predictor.
fn cohort_majority(nights: &[LabelledNight]) -> StageLabel {
    let mut counts = [0usize; 4];
    for label in nights.iter().flat_map(|night| &night.labels) {
        if let Some(index) = class_index(*label) {
            counts[index] += 1;
        }
    }
    let largest = counts.iter().copied().max().unwrap_or(0);
    CLASSES[counts
        .iter()
        .position(|&count| count == largest)
        .unwrap_or(0)]
}

impl Evaluation {
    pub fn render(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "{} nights, {} participants, {} scored epochs, {} abstentions",
            self.nights, self.participants, self.epochs, self.abstained
        );
        let _ = writeln!(
            out,
            "pooled coverage {:.1}%; mean per-night coverage {:.1}%; mean per-night accuracy {:.1}%",
            100.0 * self.epochs as f64 / (self.epochs + self.abstained).max(1) as f64,
            self.coverage_mean * 100.0,
            self.mean_night_accuracy * 100.0
        );
        if let Some((low, high)) = self.accuracy_interval {
            let _ = writeln!(
                out,
                "participant-bootstrap 95% interval for accuracy: {:.1}% to {:.1}%",
                low * 100.0,
                high * 100.0
            );
        }
        if let Some((low, high)) = self.kappa_interval {
            let _ = writeln!(
                out,
                "participant-bootstrap 95% interval for kappa: {:.3} to {:.3}",
                low, high
            );
        }
        for report in [
            &self.coverage,
            &self.closed,
            &self.coverage_three,
            &self.closed_three,
            &self.coverage_binary,
            &self.closed_binary,
        ] {
            let _ = writeln!(out, "\n{}", report.render());
        }
        let _ = writeln!(
            out,
            "\nreference points: all valid labels, compare with the closed-set view"
        );
        for baseline in &self.baselines {
            let _ = writeln!(
                out,
                "  {:<34} accuracy {:>5.1}%  kappa {:>6.3}",
                baseline.name,
                baseline.accuracy * 100.0,
                baseline.kappa
            );
        }
        let mut weak: Vec<_> = self
            .by_participant
            .iter()
            .filter(|(_, accuracy, total)| *total > 0 && *accuracy < 0.5)
            .collect();
        weak.sort_by(|a, b| a.1.total_cmp(&b.1));
        if !weak.is_empty() {
            let _ = writeln!(out, "\nparticipants below 50% agreement");
            for (id, accuracy, total) in weak.iter().take(10) {
                let _ = writeln!(
                    out,
                    "  {id:<20} {:>5.1}% of {total} epochs",
                    accuracy * 100.0
                );
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header_with_labels(labels: &[&str]) -> Vec<u8> {
        let n = labels.len();
        let mut header = vec![b' '; 256 + n * 256];
        let mut put = |text: &str, offset: usize| {
            header[offset..offset + text.len()].copy_from_slice(text.as_bytes());
        };
        put("0       ", 0);
        put(&(256 + n * 256).to_string(), 184);
        put("1", 236);
        put("30", 244);
        put(&n.to_string(), 252);
        let at = |_width: usize, previous: usize| 256 + n * previous;
        for (i, label) in labels.iter().enumerate() {
            put(label, at(16, 0) + 16 * i);
            put("uV", at(8, 16 + 80) + 8 * i);
            put("-1", at(8, 16 + 80 + 8) + 8 * i);
            put("1", at(8, 16 + 80 + 16) + 8 * i);
            put("-32768", at(8, 16 + 80 + 24) + 8 * i);
            put("32767", at(8, 16 + 80 + 32) + 8 * i);
            put("1", at(8, 16 + 160 + 40) + 8 * i);
        }
        header
    }

    #[test]
    fn respiration_channel_prefers_airflow_over_effort() {
        let edf = edf::parse_header(&header_with_labels(&[
            "EEG",
            "Thorax effort",
            "Nasal pressure",
        ]))
        .unwrap();
        assert_eq!(respiration_channel(&edf), Some(2));
    }

    #[test]
    fn respiration_channel_falls_back_to_effort_and_rejects_noise() {
        let edf = edf::parse_header(&header_with_labels(&["EEG", "Chest belt"])).unwrap();
        assert_eq!(respiration_channel(&edf), Some(1));
        let edf = edf::parse_header(&header_with_labels(&["EEG", "SpO2"])).unwrap();
        assert_eq!(respiration_channel(&edf), None);
    }

    #[test]
    fn participant_ids_strip_night_suffixes() {
        assert_eq!(participant_of("SC4001E0-PSG"), "SC400");
        assert_eq!(participant_of("SC4002E0-PSG"), "SC400");
        assert_eq!(participant_of("SC4011E0-PSG"), "SC401");
        assert_eq!(participant_of("ST7011J0-PSG"), "ST701");
        assert_eq!(participant_of("ST7012J0-PSG"), "ST701");
        assert_eq!(participant_of("mesa-sleep-0001"), "0001");
        assert_eq!(participant_of("shhs1-200001"), "200001");
    }

    #[test]
    fn evaluation_rejects_truncated_recordings() {
        let edf = edf::parse_header(&header_with_labels(&["Flow"])).unwrap();
        assert!(require_complete(&edf, edf.header_bytes).is_err());
        assert!(require_complete(&edf, edf.header_bytes + edf.record_bytes).is_ok());
    }

    #[test]
    fn cohort_majority_is_not_assumed_light_and_ties_are_stable() {
        let mut night = LabelledNight {
            id: "test".into(),
            flow: vec![0.0; HZ * 90],
            labels: vec![StageLabel::Wake, StageLabel::Wake, StageLabel::Light],
        };
        let report = evaluate(
            &[night.clone()],
            &sleeplens_core::sleep_stages::default_params(),
        );
        assert_eq!(report.baselines[1].matrix[0][0], 2);
        assert_eq!(report.baselines[1].matrix[1][0], 1);
        assert_eq!(report.baselines[0].matrix[0][1], 2);
        night.labels = vec![StageLabel::Light, StageLabel::Wake];
        assert_eq!(cohort_majority(&[night]), StageLabel::Wake);
    }

    #[test]
    fn sleep_edf_discovery_keeps_all_recording_and_scorer_letters() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("sleeplens-pairing-{}-{unique}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        for name in [
            "SC4001E0-PSG.edf",
            "SC4001EJ-Hypnogram.edf",
            "SC4261F0-PSG.edf",
            "SC4261FM-Hypnogram.edf",
            "SC4281G0-PSG.edf",
            "SC4281GC-Hypnogram.edf",
            "SC4282G0-PSG.edf",
        ] {
            std::fs::write(dir.join(name), []).unwrap();
        }
        let (pairs, unpaired) = discover(&dir, Layout::SleepEdf).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(pairs.len(), 3);
        assert_eq!(unpaired, ["SC4282G0-PSG: no hypnogram"]);
        assert!(pairs.iter().any(|p| p.id == "SC4261F0-PSG"));
        assert!(pairs.iter().any(|p| p.id == "SC4281G0-PSG"));
    }

    #[test]
    fn estimate_output_is_aligned_with_flow() {
        let flow = vec![0.0f32; HZ * 60];
        let out = estimate_flow(&flow, &sleeplens_core::sleep_stages::default_params(), "x");
        assert_eq!(out.len(), 2);
        assert!(out.iter().all(|e| e.is_some()));
    }

    #[test]
    fn layout_names_are_recognised() {
        assert_eq!(Layout::parse("sleep-edf"), Some(Layout::SleepEdf));
        assert_eq!(Layout::parse("MESA"), Some(Layout::Nsrr));
        assert_eq!(Layout::parse("ucddb"), Some(Layout::Ucddb));
        assert_eq!(Layout::parse("unknown"), None);
    }

    #[test]
    fn ucddb_stage_codes_follow_dataset_legend_not_profusion() {
        let labels = stage_codes("0\n1\n2\n3\n4\n5\n6\n7\n\nX\n").unwrap();
        assert_eq!(
            labels,
            vec![
                StageLabel::Wake,
                StageLabel::Rem,
                StageLabel::Light,
                StageLabel::Light,
                StageLabel::Deep,
                StageLabel::Deep,
                StageLabel::Excluded,
                StageLabel::Excluded,
                StageLabel::Excluded,
            ]
        );
        assert!(stage_codes("").is_err());
    }
}
