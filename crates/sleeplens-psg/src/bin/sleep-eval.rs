//! `sleep-eval` — measure SleepLens stage accuracy against polysomnography.
//!
//! ```text
//! sleep-eval run  <dir> --layout sleep-edf|nsrr [--out report.txt]
//! sleep-eval tune <dir> --layout ... [--out tuned.json]
//! sleep-eval night <edf> [--hypnogram <edf>] [--trace]
//! ```
//!
//! `run` reports the full metric set. `tune` does a coordinate search over the
//! estimator's parameters, one group at a time, keeping a change only when it
//! improves participant-level held-out kappa. `night` is a debugging aid that
//! prints one night's hypnogram next to the estimate.
//!
//! Nothing here is part of the shipped app: it reads research data from a path
//! you provide, writes only where you ask, and never touches the network.

use std::path::PathBuf;
use std::process::ExitCode;

use sleeplens_core::sleep_stages::Params;
use sleeplens_psg::runner::{self, Layout};

fn usage() -> &'static str {
    "Usage:
  sleep-eval run  <dir> --layout sleep-edf|nsrr [--out report.txt] [--limit N]
  sleep-eval tune <dir> --layout sleep-edf|nsrr [--out tuned.json] [--limit N]
  sleep-eval night <edf> --hypnogram <edf> [--trace]
  sleep-eval compare <dir> --layout ... [--out report.txt]"
}

fn flag_value(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        return Err(usage().into());
    }
    match args[0].as_str() {
        "run" => cmd_run(&args),
        "tune" => cmd_tune(&args),
        "night" => cmd_night(&args),
        "compare" => cmd_compare(&args),
        "-h" | "--help" | "help" => {
            println!("{}", usage());
            Ok(())
        }
        other => Err(format!("unknown command {other}\n{}", usage())),
    }
}

fn load_nights(args: &[String]) -> Result<Vec<sleeplens_psg::LabelledNight>, String> {
    let dir = args
        .get(1)
        .filter(|a| !a.starts_with('-'))
        .ok_or("missing <dir>")?;
    let layout_name = flag_value(args, "--layout").ok_or("missing --layout")?;
    let layout = Layout::parse(&layout_name)
        .ok_or_else(|| format!("unknown layout {layout_name}; use sleep-edf, nsrr or ucddb"))?;
    let limit: Option<usize> = flag_value(args, "--limit").and_then(|v| v.parse().ok());
    let (files, unpaired) = runner::discover(&PathBuf::from(dir), layout)?;
    if files.is_empty() {
        return Err(format!(
            "no paired recordings found in {dir} for layout {}",
            match layout {
                Layout::SleepEdf => "sleep-edf",
                Layout::Nsrr => "nsrr",
                Layout::Ucddb => "ucddb",
            }
        ));
    }
    for note in &unpaired {
        eprintln!("skipped {note}");
    }
    let mut nights = Vec::new();
    for file in files.iter().take(limit.unwrap_or(usize::MAX)) {
        match runner::load(file) {
            Ok(night) => nights.push(night),
            Err(message) => eprintln!("skipped {message}"),
        }
    }
    if nights.is_empty() {
        return Err("no night could be loaded".into());
    }
    Ok(nights)
}

fn cmd_run(args: &[String]) -> Result<(), String> {
    let nights = load_nights(args)?;
    let evaluation = runner::evaluate(&nights, &sleeplens_core::sleep_stages::default_params());
    let report = evaluation.render();
    match flag_value(args, "--out") {
        Some(path) => {
            std::fs::write(&path, &report).map_err(|e| format!("cannot write {path}: {e}"))?;
            println!("wrote {path}");
        }
        None => print!("{report}"),
    }
    Ok(())
}

/// Coordinate search over the estimator's parameter groups.
///
/// The search is deliberately simple and conservative: one parameter at a time,
/// a small multiplicative grid, and a change is kept only if it improves
/// participant-level kappa *and* does not lose more than a point of accuracy.
/// That guards against a kappa gain bought by collapsing everything into one
/// rare class.
///
/// It is not a substitute for calibration on independent data. With PSG for a
/// few dozen participants, what this establishes is the *direction* and rough
/// size of each parameter's effect, which is exactly what a hand-set heuristic
/// lacks. Nothing is written into the shipped defaults automatically.
fn cmd_tune(args: &[String]) -> Result<(), String> {
    let nights = load_nights(args)?;
    // Split participants, not nights, so repeat recordings of one person never
    // straddle the split. That leakage is the classic way to overstate stage
    // accuracy.
    let mut participants: Vec<String> = nights
        .iter()
        .map(|n| runner::participant_of(&n.id))
        .collect();
    participants.sort();
    participants.dedup();
    if participants.len() < 4 {
        return Err(format!(
            "need at least 4 participants to tune; found {}",
            participants.len()
        ));
    }
    let split = (participants.len() * 3) / 4;
    let train_names: std::collections::BTreeSet<&str> =
        participants[..split].iter().map(String::as_str).collect();
    let (train, heldout): (Vec<_>, Vec<_>) = nights
        .iter()
        .cloned()
        .partition(|n| train_names.contains(runner::participant_of(&n.id).as_str()));
    println!(
        "tuning on {} training nights / {} held-out nights from {} participants",
        train.len(),
        heldout.len(),
        participants.len()
    );

    let mut params = Params::default();
    let baseline = runner::evaluate(&heldout, &params);
    let mut best_kappa = baseline.coverage_three.kappa;
    let mut best_accuracy = baseline.coverage_three.accuracy;
    println!(
        "starting held-out three-class kappa {:.3}, accuracy {:.1}%",
        best_kappa,
        best_accuracy * 100.0
    );

    let mut accepted = Vec::new();
    for (name, apply) in candidate_adjustments() {
        let mut improved = params;
        if !apply(&mut improved) {
            continue;
        }
        if improved.validate().is_err() {
            continue;
        }
        let trained = runner::evaluate(&train, &improved);
        let held = runner::evaluate(&heldout, &improved);
        // Require the change to help on training data too; a change that only
        // helps held-out data is noise.
        if trained.coverage_three.kappa < runner::evaluate(&train, &params).coverage_three.kappa {
            continue;
        }
        if held.coverage_three.kappa > best_kappa + 1e-3
            && held.coverage_three.accuracy + 0.01 >= best_accuracy
        {
            println!(
                "  accept {name}: kappa {:.3} -> {:.3}, accuracy {:.1}% -> {:.1}%",
                best_kappa,
                held.coverage_three.kappa,
                best_accuracy * 100.0,
                held.coverage_three.accuracy * 100.0
            );
            best_kappa = held.coverage_three.kappa;
            best_accuracy = held.coverage_three.accuracy;
            params = improved;
            accepted.push(name.to_string());
        }
    }

    let final_report = runner::evaluate(&heldout, &params);
    let output = format!(
        "{}\n\naccepted adjustments ({}):\n{}\n\nfinal parameters:\n{:#?}\n",
        final_report.render(),
        accepted.len(),
        if accepted.is_empty() {
            "  none".to_string()
        } else {
            accepted
                .iter()
                .map(|name| format!("  {name}"))
                .collect::<Vec<_>>()
                .join("\n")
        },
        params
    );
    match flag_value(args, "--out") {
        Some(path) => {
            std::fs::write(&path, &output).map_err(|e| format!("cannot write {path}: {e}"))?;
            println!("wrote {path}");
        }
        None => print!("{output}"),
    }
    Ok(())
}

/// Named single-parameter adjustments, tried in order.
///
/// Each entry is a coarse step in the direction the literature suggests, not a
/// fine grid: the goal is to find which knobs matter, not to overfit.
fn candidate_adjustments() -> Vec<(&'static str, fn(&mut Params) -> bool)> {
    // Each entry is one coarse step in a direction the literature suggests, not a
    // fine grid: the aim is to find which knobs matter, not to overfit.
    fn scaled(value: &mut f64, factor: f64, range: (f64, f64)) -> bool {
        let next = *value * factor;
        if next < range.0 || next > range.1 {
            return false;
        }
        *value = next;
        true
    }
    // Scale either bound of a soft ramp; the pair keeps its ordering.
    fn scaled_band(band: &mut (f64, f64), factor: f64, range: (f64, f64)) -> bool {
        let start = band.0 * factor;
        let full = band.1 * factor;
        if start < range.0 || full > range.1 {
            return false;
        }
        *band = (start, full);
        true
    }
    vec![
        ("emission_scale x1.3", |p| {
            scaled(&mut p.emission_scale, 1.3, (0.2, 12.0))
        }),
        ("emission_scale x0.8", |p| {
            scaled(&mut p.emission_scale, 0.8, (0.2, 12.0))
        }),
        ("wake_irregular x1.3", |p| {
            scaled(&mut p.wake_irregular, 1.3, (0.2, 4.0))
        }),
        ("wake_irregular x0.8", |p| {
            scaled(&mut p.wake_irregular, 0.8, (0.2, 4.0))
        }),
        ("wake_threshold x1.25", |p| {
            scaled(&mut p.wake_threshold, 1.25, (0.05, 2.0))
        }),
        ("wake_threshold x0.8", |p| {
            scaled(&mut p.wake_threshold, 0.8, (0.05, 2.0))
        }),
        ("wake_arousal x1.3", |p| {
            scaled(&mut p.wake_arousal, 1.3, (0.0, 3.0))
        }),
        ("wake_arousal x0.7", |p| {
            scaled(&mut p.wake_arousal, 0.7, (0.0, 3.0))
        }),
        ("wake_interval_cv x0.8", |p| {
            scaled_band(&mut p.wake_interval_cv, 0.8, (0.01, 0.8))
        }),
        ("wake_interval_cv x1.25", |p| {
            scaled_band(&mut p.wake_interval_cv, 1.25, (0.01, 0.8))
        }),
        ("wake_rate x0.85", |p| {
            scaled_band(&mut p.wake_rate, 0.85, (8.0, 30.0))
        }),
        ("wake_rate x1.15", |p| {
            scaled_band(&mut p.wake_rate, 1.15, (8.0, 30.0))
        }),
        ("deep_regularity x1.25", |p| {
            scaled(&mut p.deep_regularity, 1.25, (0.2, 4.0))
        }),
        ("deep_regularity x0.8", |p| {
            scaled(&mut p.deep_regularity, 0.8, (0.2, 4.0))
        }),
        ("deep_threshold x1.25", |p| {
            scaled(&mut p.deep_threshold, 1.25, (0.05, 2.0))
        }),
        ("deep_threshold x0.8", |p| {
            scaled(&mut p.deep_threshold, 0.8, (0.05, 2.0))
        }),
        ("deep_interval_cap x0.85", |p| {
            scaled(&mut p.deep_interval_cap, 0.85, (0.02, 0.3))
        }),
        ("deep_interval_cap x1.2", |p| {
            scaled(&mut p.deep_interval_cap, 1.2, (0.02, 0.3))
        }),
        ("deep_contrast x1.25", |p| {
            scaled(&mut p.deep_contrast, 1.25, (1.2, 12.0))
        }),
        ("deep_contrast x0.8", |p| {
            scaled(&mut p.deep_contrast, 0.8, (1.2, 12.0))
        }),
        ("rem_hypoventilation x1.25", |p| {
            scaled(&mut p.rem_hypoventilation, 1.25, (0.2, 4.0))
        }),
        ("rem_hypoventilation x0.8", |p| {
            scaled(&mut p.rem_hypoventilation, 0.8, (0.2, 4.0))
        }),
        ("rem_threshold x1.25", |p| {
            scaled(&mut p.rem_threshold, 1.25, (0.05, 2.0))
        }),
        ("rem_threshold x0.8", |p| {
            scaled(&mut p.rem_threshold, 0.8, (0.05, 2.0))
        }),
        ("rem_irregular x1.25", |p| {
            scaled(&mut p.rem_irregular, 1.25, (0.1, 3.0))
        }),
        ("rem_irregular x0.8", |p| {
            scaled(&mut p.rem_irregular, 0.8, (0.1, 3.0))
        }),
        ("rem_ventilation x1.05", |p| {
            let mut band = p.rem_ventilation;
            let ok = scaled_band(&mut band, 1.05, (0.3, 1.2));
            p.rem_ventilation = band;
            ok
        }),
        ("rem_ventilation x0.95", |p| {
            let mut band = p.rem_ventilation;
            let ok = scaled_band(&mut band, 0.95, (0.3, 1.2));
            p.rem_ventilation = band;
            ok
        }),
        ("rem_wake_conflict x1.2", |p| {
            scaled(&mut p.rem_wake_conflict, 1.2, (0.0, 3.0))
        }),
        ("rem_wake_conflict x0.85", |p| {
            scaled(&mut p.rem_wake_conflict, 0.85, (0.0, 3.0))
        }),
        ("rem_slow x1.3", |p| {
            scaled(&mut p.rem_slow, 1.3, (0.0, 2.0))
        }),
        ("rem_slow x0.7", |p| {
            scaled(&mut p.rem_slow, 0.7, (0.0, 2.0))
        }),
        ("leave_cost x1.3", |p| {
            for v in p.leave_cost.iter_mut() {
                *v = (*v * 1.3).min(12.0);
            }
            true
        }),
        ("leave_cost x0.75", |p| {
            for v in p.leave_cost.iter_mut() {
                *v *= 0.75;
            }
            true
        }),
        ("enter_cost x1.3", |p| {
            for v in p.enter_cost.iter_mut() {
                *v = (*v * 1.3).min(12.0);
            }
            true
        }),
        ("enter_cost x0.75", |p| {
            for v in p.enter_cost.iter_mut() {
                *v *= 0.75;
            }
            true
        }),
        ("expected_bout x1.5", |p| {
            for v in p.expected_bout.iter_mut() {
                *v *= 1.5;
            }
            true
        }),
        ("expected_bout x0.7", |p| {
            for v in p.expected_bout.iter_mut() {
                *v = (*v * 0.7).max(1.0);
            }
            true
        }),
    ]
}

/// Compare two parameter sets on the same nights, which is what makes a change
/// a measurement rather than a guess: the same data, the same split, only the
/// estimator differing.
fn cmd_compare(args: &[String]) -> Result<(), String> {
    let nights = load_nights(args)?;
    let mut participants: Vec<String> = nights
        .iter()
        .map(|n| runner::participant_of(&n.id))
        .collect();
    participants.sort();
    participants.dedup();
    let split = (participants.len() * 3) / 4;
    let train_names: std::collections::BTreeSet<&str> =
        participants[..split].iter().map(String::as_str).collect();
    let heldout: Vec<_> = nights
        .iter()
        .filter(|n| !train_names.contains(runner::participant_of(&n.id).as_str()))
        .cloned()
        .collect();
    let heldout = if heldout.is_empty() {
        nights.clone()
    } else {
        heldout
    };
    let shipped = runner::evaluate(&heldout, &sleeplens_core::sleep_stages::default_params());
    let mut report = String::from("held-out evaluation of the shipped parameters\n\n");
    report.push_str(&shipped.render());
    match flag_value(args, "--out") {
        Some(path) => {
            std::fs::write(&path, &report).map_err(|e| format!("cannot write {path}: {e}"))?;
            println!("wrote {path}");
        }
        None => print!("{report}"),
    }
    Ok(())
}

/// Print one night's hypnogram alongside its estimate. Useful for checking a
/// specific failure instead of staring at aggregate numbers.
fn cmd_night(args: &[String]) -> Result<(), String> {
    let name = args.get(1).ok_or("missing <recording>")?;
    // Accept either a full path plus --hypnogram, or a bare id under --layout.
    let files = match flag_value(args, "--hypnogram") {
        Some(hypnogram) => runner::NightFiles {
            id: PathBuf::from(name)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| name.clone()),
            signal: runner::NightSignal::Edf(PathBuf::from(name)),
            labels: runner::NightLabels::EdfHypnogram(PathBuf::from(hypnogram)),
        },
        None => {
            let layout = flag_value(args, "--layout")
                .and_then(|l| Layout::parse(&l))
                .ok_or("missing --hypnogram or --layout")?;
            let dir = PathBuf::from(name);
            let (found, _) = runner::discover(&dir, layout)?;
            let wanted = flag_value(args, "--only");
            found
                .into_iter()
                .find(|f| wanted.as_ref().is_none_or(|w| f.id.contains(w.as_str())))
                .ok_or("no matching night")?
        }
    };
    let night = runner::load(&files)?;
    let predictions = runner::estimate(&night, &sleeplens_core::sleep_stages::default_params());
    let glyph = |stage: Option<sleeplens_psg::StageLabel>| match stage {
        Some(sleeplens_psg::StageLabel::Wake) => 'W',
        Some(sleeplens_psg::StageLabel::Light) => 'L',
        Some(sleeplens_psg::StageLabel::Deep) => 'D',
        Some(sleeplens_psg::StageLabel::Rem) => 'R',
        Some(sleeplens_psg::StageLabel::Excluded) => '?',
        None => '.',
    };
    println!("{}: {} epochs", night.id, night.labels.len());
    println!(
        "  truth    {}",
        night
            .labels
            .iter()
            .map(|l| glyph(Some(*l)))
            .collect::<String>()
    );
    let estimated: String = predictions.iter().map(|p| glyph(p.flatten())).collect();
    println!("  estimate {estimated}");
    Ok(())
}
