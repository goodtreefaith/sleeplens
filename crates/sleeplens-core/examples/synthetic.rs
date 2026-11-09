//! Print hypnograms for synthetic breathing profiles with known stages.
//!
//! A development aid for eyeballing estimator behaviour on cases whose correct
//! answer is known by construction, and for checking that a change did what it
//! intended. It is not evidence of accuracy: synthetic breathing is not sleep.
//! Measured accuracy comes from `cargo run -p sleeplens-psg --bin sleep-eval`.
use sleeplens_core::sleep_stages::{estimate_sleep_stages, SleepStage, SleepStageInput};
use std::f32::consts::TAU;

const HZ: u32 = 50;

fn random(seed: &mut u32) -> f32 {
    *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    (*seed >> 8) as f32 / 16_777_215.0 * 2.0 - 1.0
}

/// (period, interval jitter, amplitude, volume jitter, noise) per minute.
fn breathing(minutes: usize, profile: impl Fn(usize) -> (f32, f32, f32, f32, f32)) -> Vec<f32> {
    let samples = minutes * 60 * HZ as usize;
    let mut flow = Vec::with_capacity(samples);
    let mut seed = 7;
    while flow.len() < samples {
        let minute = flow.len() / (60 * HZ as usize);
        let (period, jitter, amplitude, volume_jitter, noise) = profile(minute);
        let length = ((period * (1.0 + jitter * random(&mut seed))) * HZ as f32) as usize;
        let amplitude = amplitude * (1.0 + volume_jitter * random(&mut seed));
        for i in 0..length {
            if flow.len() == samples {
                break;
            }
            let value =
                amplitude * (TAU * i as f32 / length as f32).sin() + noise * random(&mut seed);
            flow.push(value);
        }
    }
    flow
}

fn show(label: &str, flow: &[f32]) {
    let result = estimate_sleep_stages(&SleepStageInput {
        start: "2030-01-15T22:00:00",
        flow,
        hz: HZ,
        excess_leak: &[],
        event_offsets: &[],
    });
    let line: String = result
        .stages
        .iter()
        .map(|s| match s {
            SleepStage::Wake => 'W',
            SleepStage::Light => 'L',
            SleepStage::Deep => 'D',
            SleepStage::Rem => 'R',
            SleepStage::Uncertain => 'U',
            SleepStage::NoData => '-',
        })
        .collect();
    println!("{label}  ({:.0} min)", result.stages.len() as f32 * 0.5);
    for chunk in line.as_bytes().chunks(60) {
        println!("  {}", std::str::from_utf8(chunk).unwrap());
    }
    println!(
        "  wake {:.1} light {:.1} deep {:.1} rem {:.1} uncertain {:.1}",
        result.minutes.wake,
        result.minutes.light,
        result.minutes.deep,
        result.minutes.rem,
        result.minutes.uncertain
    );
}

fn main() {
    show(
        "steady regular",
        &breathing(60, |_| (4.0, 0.01, 22.0, 0.01, 0.0)),
    );
    show(
        "regular 20-60 min",
        &breathing(90, |minute| {
            if minute >= 20 {
                (4.0, 0.01, 22.0, 0.01, 0.0)
            } else {
                (4.0, 0.15, 22.0, 0.12, 0.0)
            }
        }),
    );
    show(
        "REM-like 30-40 min",
        &breathing(60, |minute| {
            if (30..40).contains(&minute) {
                (4.5, 0.27, 14.0, 0.30, 0.0)
            } else {
                (4.0, 0.05, 22.0, 0.05, 0.0)
            }
        }),
    );
    show(
        "irregular with sighs 30-40 min",
        &breathing(60, |minute| {
            if (30..40).contains(&minute) {
                (4.0, 0.22, 22.0, 0.30, 0.0)
            } else {
                (4.0, 0.05, 22.0, 0.06, 0.0)
            }
        }),
    );
}
