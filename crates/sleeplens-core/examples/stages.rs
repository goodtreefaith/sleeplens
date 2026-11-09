//! Print breathing-based stage estimates without printing device identifiers.
//! Run with `cargo run --release -p sleeplens-core --example stages -- <folder> [--last N] [--debug]`.
use sleeplens_core::{
    bkp::{parse_bkp, TherapyEvent},
    dat::iterate_dat_records,
    sleep_stages::{estimate_sleep_stages, sleep_stage_debug, SleepStage, SleepStageInput},
};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

const HZ: usize = 50;

// Error messages deliberately omit paths, which can contain device identifiers.
fn discover(
    root: &Path,
    dat: &mut Vec<PathBuf>,
    bkp: &mut Vec<PathBuf>,
) -> Result<(), &'static str> {
    for entry in fs::read_dir(root).map_err(|_| "Cannot list card folder")? {
        let entry = entry.map_err(|_| "Cannot read folder entry")?;
        let kind = entry
            .file_type()
            .map_err(|_| "Cannot inspect folder entry")?;
        if kind.is_symlink() || entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        let path = entry.path();
        if kind.is_dir() {
            discover(&path, dat, bkp)?;
        } else if kind.is_file() {
            match path
                .extension()
                .and_then(|s| s.to_str())
                .map(str::to_ascii_lowercase)
                .as_deref()
            {
                Some("dat") => dat.push(path),
                Some("bkp") => bkp.push(path),
                _ => {}
            }
        }
    }
    Ok(())
}

fn date(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_str()?;
    if stem.len() != 8 || !stem.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(format!("{}-{}-{}", &stem[..4], &stem[4..6], &stem[6..]))
}

fn stage_index(stage: SleepStage) -> usize {
    match stage {
        SleepStage::Wake => 0,
        SleepStage::Light => 1,
        SleepStage::Deep => 2,
        SleepStage::Rem => 3,
        SleepStage::NoData => 4,
    }
}

fn print_night(
    path: &Path,
    date: &str,
    events: &[TherapyEvent],
    debug: bool,
) -> Result<(), &'static str> {
    let bytes = fs::read(path).map_err(|_| "Cannot read night file")?;
    let mut flow = Vec::new();
    let mut leak = Vec::new();
    let mut start = None;
    for record in iterate_dat_records(&bytes, true) {
        let Some(time) = record.time else {
            continue;
        };
        let first = start.get_or_insert_with(|| time.clone());
        let offset = time.epoch_sec - first.epoch_sec;
        // Reject corrupt chronology before allocating a potentially huge gap.
        if !(0..=48 * 60 * 60).contains(&offset) {
            return Err("Night timestamps exceed the supported 48-hour span");
        }
        let second = offset as usize;
        flow.resize(flow.len().max((second + 1) * HZ), f32::NAN);
        leak.resize(leak.len().max(second + 1), f32::NAN);
        if let Some(waveforms) = record.waveforms {
            flow[second * HZ..(second + 1) * HZ].copy_from_slice(&waveforms.patient_flow);
            leak[second] = record.excess_leak as f32;
        }
    }
    let Some(start) = start else {
        println!("{date}: no usable waveform records");
        return Ok(());
    };
    // Keep wall-clock alignment. The occasional one-second device clock skip
    // stays unavailable; only long missing runs become NoData scoring epochs.
    let event_offsets: Vec<_> = events
        .iter()
        .filter(|event| {
            matches!(
                event.event_type.as_str(),
                "obstructiveApnea" | "centralApnea" | "hypopnea"
            )
        })
        .filter_map(|event| {
            let offset = event.time.epoch_sec - start.epoch_sec;
            (offset >= 0 && offset < leak.len() as i64).then_some(offset as u32)
        })
        .collect();
    let input = SleepStageInput {
        start: &start.iso,
        flow: &flow,
        hz: HZ as u32,
        excess_leak: &leak,
        event_offsets: &event_offsets,
    };
    let result = estimate_sleep_stages(&input);
    let minutes = [
        result.minutes.wake,
        result.minutes.light,
        result.minutes.deep,
        result.minutes.rem,
    ];
    let masked_on = flow.iter().filter(|v| v.is_finite()).count() as f32 / HZ as f32 / 60.0;
    print!("{date} mask-on {masked_on:.1} min");
    for (label, value) in ["wake", "light", "deep", "rem"].into_iter().zip(minutes) {
        let percent = if masked_on > 0.0 {
            value / masked_on * 100.0
        } else {
            0.0
        };
        print!(" | {label} {value:.1} min ({percent:.1}%)");
    }
    println!(" | quality {:.2}", result.signal_quality);
    let hypnogram: String = result
        .stages
        .chunks(10)
        .map(|chunk| {
            let mut counts = [0usize; 5];
            for &stage in chunk {
                counts[stage_index(stage)] += 1;
            }
            let index = (0..5).max_by_key(|&i| counts[i]).unwrap_or(4);
            ['W', 'L', 'D', 'R', '-'][index]
        })
        .collect();
    println!("  {hypnogram}");
    if debug {
        println!("{}", sleep_stage_debug(&input));
    }
    Ok(())
}

fn run() -> Result<(), &'static str> {
    let mut args = env::args_os().skip(1);
    let mut root = None;
    let mut last = 7;
    let mut debug = false;
    while let Some(arg) = args.next() {
        if arg == "--debug" {
            debug = true;
        } else if arg == "--last" {
            last = args
                .next()
                .and_then(|s| s.to_str().and_then(|s| s.parse::<usize>().ok()))
                .filter(|&n| n > 0)
                .ok_or("--last requires a positive integer")?;
        } else if arg.to_string_lossy().starts_with('-') || root.is_some() {
            return Err("Usage: stages <card folder> [--last N] [--debug]");
        } else {
            root = Some(PathBuf::from(arg));
        }
    }
    let root = root.ok_or("Usage: stages <card folder> [--last N] [--debug]")?;
    if fs::symlink_metadata(&root)
        .map_err(|_| "Cannot inspect card folder")?
        .file_type()
        .is_symlink()
    {
        return Err("Card folder must not be a symbolic link");
    }
    let root = fs::canonicalize(root).map_err(|_| "Cannot resolve card folder")?;
    let (mut dat, mut bkp) = (Vec::new(), Vec::new());
    discover(&root, &mut dat, &mut bkp)?;
    let mut nights: Vec<_> = dat
        .into_iter()
        .filter_map(|p| date(&p).map(|d| (d, p)))
        .collect();
    nights.sort();
    let mut events = Vec::new();
    for path in bkp {
        let bytes = fs::read(path).map_err(|_| "Cannot read backup event file")?;
        events.extend(parse_bkp(&bytes).events);
    }
    if nights.is_empty() {
        return Err("No dated DAT night files found");
    }
    println!("Estimated stages from breathing only. Hypnogram: W wake, L light, D deep, R REM, - no data; one character per five minutes.");
    for (date, path) in nights.iter().skip(nights.len().saturating_sub(last)) {
        print_night(path, date, &events, debug)?;
    }
    Ok(())
}

fn main() {
    if let Err(message) = run() {
        eprintln!("{message}");
        std::process::exit(1);
    }
}
