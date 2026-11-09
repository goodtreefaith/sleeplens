//! Breath detection, per-epoch statistics and windowed breathing features.
//!
//! This module is measurement only. It turns a patient-flow trace into
//! individual breaths and then into the timing, volume and shape statistics
//! that the scorer in `mod.rs` reasons about. Nothing here decides a stage, so
//! thresholds can be reviewed and tuned without touching the staging model.
//!
//! # How a breath is found
//!
//! PAP patient flow is a zero-mean signal with slow offset drift from leak
//! compensation, so every sample is first referred to a 31-second moving median.
//! A breath then runs from one upward crossing of the baseline to the next, with
//! hysteresis proportional to the recent amplitude:
//!
//! ```text
//!          peak
//!           ^
//!          / \          inspiratory time: upward crossing of zero to zero
//!     ____/   \____     cycle length: upward crossing to upward crossing
//!   0                 ^
//! ```
//!
//! Scaling the threshold with amplitude matters: fixed thresholds either miss
//! shallow hypopnoeic breathing or fragment large recovery breaths. The
//! inspiratory phase is measured to the *zero* crossing, not to the peak,
//! because inspiratory duty cycle is defined on zero crossings. Measuring to the
//! peak halves it and pushes every epoch below the normal range.

/// Breaths outside these bounds are treated as artefact rather than breathing.
const MIN_BREATH_SECONDS: f64 = 1.2;
const MAX_BREATH_SECONDS: f64 = 15.0;
/// Below this inspired volume a "breath" is indistinguishable from drift.
const MIN_INSPIRED_LITRES: f64 = 0.015;
/// Flow beyond this is not physiological through a PAP circuit.
const MAX_PLAUSIBLE_FLOW: f32 = 150.0;

/// One detected breath, in the units the scorer uses.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Breath {
    /// Total cycle length in seconds.
    pub duration: f64,
    /// Inspiratory time in seconds, zero crossing to zero crossing.
    pub inspiratory: f64,
    /// Inspired volume in litres above the local baseline.
    pub volume: f64,
    /// Peak inspiratory flow above the local baseline, L/min.
    pub peak: f64,
    /// Inspiratory time divided by cycle length.
    pub duty: f64,
    /// Mean inspiratory flow over the middle of inspiration, relative to peak.
    /// About 0.94 for a sine; near 1.0 for a flattened, flow-limited contour.
    pub squareness: f64,
    /// A plateau at the peak suggests clipping or flow limitation.
    pub flat_top: bool,
}

impl Breath {
    /// Flow-limitation evidence in 0..1.
    ///
    /// A sinusoidal inspiration has a mid-inspiratory mean of about 0.94 of its
    /// peak. A flat-topped, flow-limited contour pushes that towards 1.0, and a
    /// sustained plateau at the peak is the clearest sign of all.
    pub fn limitation(&self) -> f64 {
        let flattened = ((self.squareness - 0.95) / 0.05).clamp(0.0, 1.0);
        if self.flat_top {
            flattened.max(0.7)
        } else {
            flattened * 0.6
        }
    }
}

/// Accumulated evidence for one 30-second scoring epoch.
#[derive(Default, Clone)]
pub(crate) struct Epoch {
    pub valid: usize,
    pub artifacts: usize,
    pub clipped: usize,
    pub energy: f64,
    pub noise: f64,
    /// Seconds of the epoch occupied by detected breaths.
    pub covered: f64,
    pub leak_samples: usize,
    pub large_leak: usize,
    /// A machine-scored apnoea or hypopnoea ended in or near this epoch.
    pub event: bool,
    /// 0..1: how strongly a machine event suggests an arousal here.
    pub arousal: f64,
    pub breaths: Vec<Breath>,
}

/// Expiration depth that must be reached before a new breath can start, as a
/// fraction of the recent flow amplitude.
///
/// A *relative* depth is essential. Channels differ by orders of magnitude
/// between devices - ucddb flow peaks around 0.2 units where an AS100 peaks at
/// 20 L/min - so a fixed floor in litres either swallows a shallow trace whole
/// (every epoch abstains) or admits every wobble. The absolute floor exists only
/// to stop a genuinely flat signal from registering endless breaths.
const EXPIRATORY_DEPTH: f64 = 0.35;
const MIN_EXPIRATORY_DEPTH: f64 = 0.002;

pub(crate) struct Signal {
    pub epoch: Vec<Epoch>,
    /// Five-second RMS envelope of the filtered flow, for periodic-breathing checks.
    pub envelope: Vec<Option<f64>>,
}

/// Breathing features pooled over a window of neighbouring epochs.
///
/// Two scales are used: a short window that follows breath-to-breath detail and
/// a long window that captures the slower changes distinguishing sustained
/// wakefulness or REM from a brief disturbance.
#[derive(Default, Clone, Copy)]
pub(crate) struct Window {
    pub count: usize,
    pub interval_cv: f64,
    pub volume_cv: f64,
    pub ventilation_cv: f64,
    pub duty_cv: f64,
    pub duty: f64,
    pub rate: f64,
    pub volume: f64,
    pub ventilation: f64,
    /// Largest breath in the window, litres.
    pub largest: f64,
    /// Median mid-inspiratory flow relative to peak: a flow-limitation marker.
    pub squareness: f64,
    /// Spread of the per-epoch median ventilation, not of single breaths.
    /// Slow oscillations show up here but not in breath-to-breath CV.
    pub slow_ventilation_cv: f64,
    /// Breaths at least 1.9x the window median volume.
    pub sighs: usize,
    /// Median peak inspiratory flow. A drop here with unchanged volume means a
    /// longer, flatter inspiration, which is the obstructive pattern.
    #[allow(dead_code)]
    pub peak: f64,
    /// Spread of per-breath peak flow: breaths driven at very different efforts,
    /// which distinguishes REM drive variability from obstructive narrowing.
    pub peak_cv: f64,
    /// Median inspiratory time, seconds.
    #[allow(dead_code)]
    pub inspiratory: f64,
}

pub(crate) fn percentile(values: &[f64], fraction: f64, fallback: f64) -> f64 {
    if values.is_empty() {
        return fallback;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable_by(f64::total_cmp);
    sorted[((sorted.len() - 1) as f64 * fraction).round() as usize]
}

pub(crate) fn median(values: &[f64], fallback: f64) -> f64 {
    percentile(values, 0.5, fallback)
}

/// Interquartile range divided by 1.349 leaves a robust standard deviation,
/// which is then expressed relative to the median. Outlying breaths barely move
/// it, unlike a plain coefficient of variation.
pub(crate) fn robust_cv(values: &[f64]) -> f64 {
    if values.len() < 3 {
        return 0.0;
    }
    let centre = median(values, 0.0);
    if !centre.is_finite() || centre <= 0.0 {
        return 0.0;
    }
    let spread = percentile(values, 0.75, 0.0) - percentile(values, 0.25, 0.0);
    (spread / (1.349 * centre)).max(0.0)
}

/// Robust location and relative spread of the same values.
fn robust(values: &[f64]) -> (f64, f64) {
    (median(values, 0.0), robust_cv(values))
}

/// Per-second *mean* flow, then a moving median over two minutes.
///
/// PAP patient flow carries a slow offset from leak compensation, which drifts
/// over minutes, not seconds. Two details matter and both were got wrong first:
///
/// * The per-second statistic must be a **mean**, not a median. With breaths of
///   three to five seconds, a median over exactly one second samples the same
///   phase of every breath, so the "baseline" is a phase-locked copy of the
///   breathing signal and subtracting it corrupts the waveform.
/// * The smoothing window must span **many breaths**. A 31-second window holds
///   about eight cycles, and because that is not an integer the median ripples
///   with breathing phase, which modulates every breath's measured volume and
///   fabricates variability in perfectly regular breathing. Two minutes holds
///   around thirty breaths.
fn baseline(flow: &[f32], hz: usize) -> Vec<f64> {
    const HALF_WINDOW: usize = 60;
    let seconds = flow.len().div_ceil(hz);
    let mut per_second = Vec::with_capacity(seconds);
    for second in 0..seconds {
        let lo = second * hz;
        let hi = (lo + hz).min(flow.len());
        let mut sum = 0.0;
        let mut count = 0usize;
        for value in &flow[lo..hi] {
            if value.is_finite() {
                sum += f64::from(*value);
                count += 1;
            }
        }
        per_second.push((count > 0).then(|| sum / count as f64));
    }
    let mut smoothed = vec![0.0; seconds];
    let mut window: Vec<f64> = Vec::with_capacity(HALF_WINDOW * 2 + 1);
    for second in 0..seconds {
        window.clear();
        let lo = second.saturating_sub(HALF_WINDOW);
        let hi = (second + HALF_WINDOW + 1).min(seconds);
        window.extend(per_second[lo..hi].iter().flatten().copied());
        smoothed[second] = if window.is_empty() {
            0.0
        } else {
            window.iter().sum::<f64>() / window.len() as f64
        };
    }
    smoothed
}

pub(crate) fn analyze(
    flow: &[f32],
    hz: usize,
    excess_leak: &[f32],
    event_offsets: &[u32],
    per_epoch: usize,
) -> Signal {
    let epochs = flow.len().div_ceil(per_epoch);
    let mut local = vec![Epoch::default(); epochs];
    if hz == 0 || per_epoch == 0 {
        return Signal {
            epoch: local,
            envelope: Vec::new(),
        };
    }

    // ---- Per-sample statistics and the centred signal ----------------------
    //
    // The filter is deliberately light. A 100 ms low-pass was tried first and
    // measurably hurt: it shrinks and phase-shifts the waveform, so breath
    // boundaries land at a slightly different phase for every breath and
    // perfectly regular breathing then *appears* to vary in volume. Only
    // single-sample quantisation noise needs removing.
    let offset = baseline(flow, hz);
    let alpha = 1.0 / (1.0 + 0.02 * hz as f64);
    let mut filtered = 0.0_f64;
    let mut initialized = false;
    let mut previous_raw = 0.0_f32;
    let mut flat_start = 0_usize;
    let mut centred = vec![f64::NAN; flow.len()];
    let mut amplitude = 0.0_f64;
    let decay = (-1.0 / (12.0 * hz as f64)).exp();
    let mut depths = vec![0.0_f64; flow.len()];

    let bins = flow.len().div_ceil(hz * 5);
    let mut envelope_energy = vec![(0.0, 0usize); bins];
    for (i, &raw) in flow.iter().enumerate() {
        if !raw.is_finite() {
            initialized = false;
            continue;
        }
        // AS100 flow is stored in whole L/min, so short runs of one repeated
        // value are normal. Only an exact run longer than five seconds is
        // flat-line evidence, and the whole run is credited to each epoch it
        // crosses, including across an epoch boundary.
        if !initialized || raw != previous_raw {
            flat_start = i;
        }
        previous_raw = raw;
        let flat_length = i - flat_start + 1;
        if flat_length == hz * 5 + 1 {
            let first = flat_start / per_epoch;
            let last = i / per_epoch;
            for (epoch, f) in local.iter_mut().enumerate().take(last + 1).skip(first) {
                let lo = flat_start.max(epoch * per_epoch);
                let hi = (i + 1).min((epoch + 1) * per_epoch);
                f.clipped += hi - lo;
            }
        } else if flat_length > hz * 5 + 1 {
            local[i / per_epoch].clipped += 1;
        }

        let epoch = &mut local[i / per_epoch];
        epoch.valid += 1;
        epoch.artifacts += usize::from(raw.abs() > MAX_PLAUSIBLE_FLOW);

        let sample = f64::from(raw.clamp(-200.0, 200.0));
        if !initialized {
            filtered = sample;
            initialized = true;
        }
        filtered += alpha * (sample - filtered);
        epoch.energy += filtered * filtered;
        epoch.noise += (sample - filtered).powi(2);
        let bin = &mut envelope_energy[i / (hz * 5)];
        bin.0 += filtered * filtered;
        bin.1 += 1;

        let x = filtered - offset[(i / hz).min(offset.len().saturating_sub(1))];
        centred[i] = x;
        // Running maximum magnitude, decaying over about twelve seconds. This is
        // the local amplitude scale. It is a decay rather than an instantaneous
        // value so one sample of noise cannot set it, and it is used for the
        // artifact threshold rather than for breath segmentation.
        amplitude = (amplitude * decay).max(x.abs());
        // A breath must reach a real expiration before the next inspiration
        // counts, so a couple of samples either side of zero cannot split one
        // breath into two.
        //
        // The depth is a fraction of the *recent amplitude* with a very small
        // floor. A large floor was the original bug on real data: an 8 Hz channel
        // whose whole excursion is 0.2 units never reached a 0.5-unit floor, so no
        // breath ever completed and nearly every epoch abstained.
        depths[i] = (EXPIRATORY_DEPTH * amplitude).max(MIN_EXPIRATORY_DEPTH);
    }

    // ---- Segmentation ------------------------------------------------------
    //
    // One breath runs from an upward zero crossing to the next, provided the flow
    // actually dipped below its expiratory depth in between. Each upward crossing
    // is detected once in a single forward pass, which is what keeps boundaries
    // from drifting: a state machine that re-reads the same sample in two roles
    // can close and reopen a cycle at adjacent samples.
    let mut crossing = 0usize;
    let mut last_crossing: Option<usize> = None;
    let mut reached_expiration = false;
    let mut peak = 0.0_f64;
    let mut volume = 0.0_f64;
    let mut last_positive = 0usize;
    let mut flat_longest = 0usize;
    let mut inspiration: Vec<f64> = Vec::with_capacity(hz * 16);
    let litres_per_unit = 1.0 / (60.0 * hz as f64);
    let dt = 1.0 / hz as f64;
    while crossing < centred.len() {
        let x = centred[crossing];
        if x.is_nan() {
            last_crossing = None;
            reached_expiration = false;
            crossing += 1;
            continue;
        }
        if x <= 0.0 {
            crossing += 1;
            continue;
        }
        // An upward crossing. Close the previous cycle if it expired properly.
        if let Some(start) = last_crossing {
            if reached_expiration {
                if let Some(breath) = finish_breath(
                    crossing,
                    start,
                    last_positive,
                    peak,
                    volume,
                    flat_longest,
                    &inspiration,
                    dt,
                    hz,
                ) {
                    let centre = start + (last_positive.saturating_sub(start)) / 2;
                    let target = &mut local[(centre / per_epoch).min(epochs - 1)];
                    target.covered += breath.duration;
                    target.breaths.push(breath);
                }
            }
        }
        // Begin the new cycle at this crossing.
        last_crossing = Some(crossing);
        reached_expiration = false;
        peak = x;
        volume = x * litres_per_unit;
        last_positive = crossing;
        flat_longest = 0;
        inspiration.clear();
        inspiration.push(x);
        crossing += 1;
        // Consume the rest of this inspiration and its expiration.
        while crossing < centred.len() {
            let x = centred[crossing];
            if x.is_nan() {
                break;
            }
            // Once the breath has expired, the next non-negative sample begins the
            // following breath: leave it for the outer loop, which owns cycle
            // boundaries. Testing before recording keeps the boundary sample out of
            // the previous breath; counting it would make inspiration span the
            // whole cycle and duty cycle read 1.0 everywhere.
            if reached_expiration && x >= 0.0 {
                crossing += 1;
                break;
            }
            if x > 0.0 {
                last_positive = crossing;
                volume += x * litres_per_unit;
                if x > peak {
                    peak = x;
                }
                if inspiration.len() < hz * 16 {
                    inspiration.push(x);
                }
                if peak > 0.0 && x >= peak * 0.98 {
                    // A run of samples at the peak: clipping or flow limitation.
                    flat_longest += 1;
                }
            } else {
                // Expiration depth for this breath, as a fraction of its own
                // inspiratory peak but capped by the run's typical scale.
                //
                // Using the running peak alone is what failed on real recordings:
                // a single large excursion inside one cycle sets `peak` so high
                // that the following expiration never reaches the depth, no
                // expiration is ever registered, and *no breaths are detected at
                // all* for the whole night. Clamping the depth to a small multiple
                // of the signal's typical excursion keeps one artefact from
                // disabling detection.
                let depth = (EXPIRATORY_DEPTH * peak).min(EXPIRATORY_DEPTH * 3.0 * amplitude);
                let breath_depth = depth.max(MIN_EXPIRATORY_DEPTH);
                if x < -breath_depth {
                    reached_expiration = true;
                }
            }
            crossing += 1;
            if (crossing.saturating_sub(last_crossing.unwrap_or(crossing)) as f64) * dt
                > MAX_BREATH_SECONDS * 1.5
            {
                // Not breathing: abandon the partial cycle rather than merging it
                // into the next breath.
                last_crossing = None;
                reached_expiration = false;
                break;
            }
        }
    }

    for (second, &leak) in excess_leak.iter().enumerate() {
        let Some(epoch) = local.get_mut(second / 30) else {
            break;
        };
        if leak.is_finite() {
            epoch.leak_samples += 1;
            epoch.large_leak += usize::from(leak > 24.0);
        }
    }
    // A machine event ends in an arousal more often than not, but the epoch
    // holding the event is usually still sleep. So the confounder window is the
    // event itself plus the recovery immediately after it, and arousal evidence
    // decays further out. Marking the event epoch *and* the recovery as
    // uninterpretable is deliberate: recovery breaths are large and irregular and
    // would otherwise be read as REM or wake, which is the specific confusion the
    // evidence review warns about.
    for &offset in event_offsets {
        // `event_offsets` are seconds from the start, but the loop indexes
        // samples. Converting is essential: comparing them directly would place
        // every event about fifty times too early at 50 Hz and flag the wrong
        // stretch of night entirely.
        let end = offset as usize * hz;
        for (epoch, f) in local.iter_mut().enumerate() {
            let lo = epoch * per_epoch;
            let hi = lo + per_epoch;
            // Events are reported at their end. The confounded span runs from the
            // epoch holding the event start (one epoch before `end`, since events
            // last about 10-30 s) through the three epochs after it, where recovery
            // breaths sit. These are the epochs most likely to be mistaken for REM,
            // and no airflow-only method can say which stage an arousal returns to.
            if hi > end.saturating_sub(per_epoch * 2) && lo < end + per_epoch * 3 {
                f.event = true;
            }
            let weight = if end + 8 * hz <= lo {
                1.0 - ((lo - (end + 8 * hz)) as f64 / (40.0 * hz as f64)).clamp(0.0, 1.0)
            } else if end < hi {
                0.6
            } else {
                0.0
            };
            f.arousal = f.arousal.max(weight);
        }
    }

    let envelope = envelope_energy
        .iter()
        .map(|&(energy, count)| (count >= hz * 4).then(|| (energy / count as f64).sqrt()))
        .collect();
    Signal {
        epoch: local,
        envelope,
    }
}
/// Turn one completed cycle into a breath, or reject it as artefact.
#[allow(clippy::too_many_arguments)]
fn finish_breath(
    end: usize,
    cycle_start: usize,
    last_positive: usize,
    peak: f64,
    volume: f64,
    flat_longest: usize,
    inspiration: &[f64],
    dt: f64,
    hz: usize,
) -> Option<Breath> {
    let duration = (end - cycle_start) as f64 * dt;
    if !(MIN_BREATH_SECONDS..=MAX_BREATH_SECONDS).contains(&duration) {
        return None;
    }
    if volume < MIN_INSPIRED_LITRES {
        return None;
    }
    // The inspiratory phase ends at the last positive sample, not at the peak.
    // Measuring to the peak halves duty cycle and pushes every epoch below the
    // physiological range.
    let inspiratory = ((last_positive.saturating_sub(cycle_start) + 1) as f64 * dt).min(duration);
    let squareness = if inspiration.len() >= 4 && peak > 0.0 {
        let lo = (inspiration.len() as f64 * 0.3) as usize;
        let hi = ((inspiration.len() as f64 * 0.7) as usize)
            .min(inspiration.len())
            .max(lo + 1);
        let middle = &inspiration[lo..hi];
        (middle.iter().sum::<f64>() / middle.len() as f64 / peak).clamp(0.0, 1.5)
    } else {
        0.94
    };
    Some(Breath {
        duration,
        inspiratory,
        volume,
        peak,
        duty: (inspiratory / duration).clamp(0.0, 1.0),
        squareness,
        flat_top: flat_longest >= (hz as f64 * 0.8) as usize,
    })
}

pub(crate) fn window(
    epochs: &[Epoch],
    usable: &[bool],
    e: usize,
    before: usize,
    after: usize,
) -> Window {
    let mut lo = e;
    let mut hi = e;
    while lo > e.saturating_sub(before) && epochs[lo - 1].valid > 0 && usable[lo - 1] {
        lo -= 1;
    }
    while hi + 1 < epochs.len().min(e + after + 1) && epochs[hi + 1].valid > 0 && usable[hi + 1] {
        hi += 1;
    }
    let breaths: Vec<&Breath> = (lo..=hi)
        .filter(|&i| usable[i])
        .flat_map(|i| &epochs[i].breaths)
        .collect();
    if breaths.is_empty() {
        return Window::default();
    }

    let values = |get: fn(&Breath) -> f64| breaths.iter().map(|b| get(b)).collect::<Vec<_>>();
    let (_, interval_cv) = robust(&values(|b| b.duration));
    let (volume, volume_cv) = robust(&values(|b| b.volume));
    let (ventilation, ventilation_cv) = robust(&values(|b| b.volume * 60.0 / b.duration));
    let (duty, duty_cv) = robust(&values(|b| b.duty));
    let (peak, peak_cv) = robust(&values(|b| b.peak));
    let duration = median(&values(|b| b.duration), 4.0);
    let squareness = median(&values(|b| b.squareness), 0.94);
    let inspiratory = median(&values(|b| b.inspiratory), 2.0);
    let sighs = breaths.iter().filter(|b| b.volume > volume * 1.9).count();
    // Slow oscillation: spread of per-epoch median ventilation. Breath-level CV
    // cannot separate a steady irregular pattern from a slowly waxing one.
    let per_epoch: Vec<f64> = (lo..=hi)
        .filter(|&i| usable[i] && epochs[i].breaths.len() >= 2)
        .map(|i| {
            let epoch = &epochs[i].breaths;
            median(
                &epoch
                    .iter()
                    .map(|b| b.volume * 60.0 / b.duration)
                    .collect::<Vec<_>>(),
                0.0,
            )
        })
        .collect();
    Window {
        count: breaths.len(),
        interval_cv,
        volume_cv,
        ventilation_cv,
        duty_cv,
        duty,
        rate: 60.0 / duration.max(1.0),
        volume,
        ventilation,
        largest: breaths.iter().map(|b| b.volume).fold(0.0, f64::max),
        squareness,
        peak,
        peak_cv,
        inspiratory,
        slow_ventilation_cv: if per_epoch.len() >= 3 {
            robust_cv(&per_epoch)
        } else {
            0.0
        },
        sighs,
    }
}

/// Crescendo-decrescendo breathing: a five-second RMS envelope that repeats
/// every 40 to 90 seconds and reverses phase half a cycle later.
///
/// Three cycles, a strong autocorrelation and an opposite half-cycle are all
/// required, so ordinary irregularity or a single deep breath cannot trigger it.
pub(crate) fn periodic_breathing(envelope: &[Option<f64>], epochs: usize) -> Vec<bool> {
    (0..epochs)
        .map(|epoch| {
            let centre = epoch * 6 + 3;
            let lo = centre.saturating_sub(30);
            let hi = envelope.len().min(centre + 30);
            if hi <= lo {
                return false;
            }
            let slice = &envelope[lo..hi];
            if slice.len() < 24 || slice.iter().any(Option::is_none) {
                return false;
            }
            let values: Vec<f64> = slice.iter().flatten().copied().collect();
            let middle = median(&values, 0.0);
            if middle < 0.5
                || percentile(&values, 0.9, 0.0) - percentile(&values, 0.1, 0.0) < middle * 0.55
            {
                return false;
            }
            let correlation = |lag: usize| {
                let a = &values[..values.len() - lag];
                let b = &values[lag..];
                let ma = a.iter().sum::<f64>() / a.len() as f64;
                let mb = b.iter().sum::<f64>() / b.len() as f64;
                let (mut cross, mut ea, mut eb) = (0.0, 0.0, 0.0);
                for (&x, &y) in a.iter().zip(b) {
                    cross += (x - ma) * (y - mb);
                    ea += (x - ma).powi(2);
                    eb += (y - mb).powi(2);
                }
                cross / (ea * eb).sqrt().max(0.001)
            };
            (8..=18).any(|lag| {
                values.len() >= lag * 3 && correlation(lag) > 0.72 && correlation(lag / 2) < -0.35
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    const TEST_HZ: usize = 50;

    fn sine(minutes: usize, period: f32, amplitude: f32) -> Vec<f32> {
        let samples = minutes * 60 * TEST_HZ;
        let length = (period * TEST_HZ as f32).round() as usize;
        (0..samples)
            .map(|i| {
                let phase = (i % length) as f32 / length as f32;
                amplitude * (TAU * phase).sin()
            })
            .collect()
    }

    fn breaths(signal: &Signal) -> Vec<Breath> {
        signal
            .epoch
            .iter()
            .flat_map(|e| &e.breaths)
            .copied()
            .collect()
    }

    #[test]
    fn steady_sine_yields_uniform_breaths() {
        // Four-second breaths, ten minutes, no noise and no jitter. Every breath
        // must have the same cycle length, the same volume and a duty cycle near
        // 0.5. Anything else means the detector is drifting, which would show up
        // downstream as fabricated stage evidence.
        let signal = analyze(&sine(10, 4.0, 22.0), TEST_HZ, &[], &[], TEST_HZ * 30);
        let found = breaths(&signal);
        assert!(found.len() >= 140, "only {} breaths", found.len());
        let durations: Vec<f64> = found.iter().map(|b| b.duration).collect();
        let volumes: Vec<f64> = found.iter().map(|b| b.volume).collect();
        let duties: Vec<f64> = found.iter().map(|b| b.duty).collect();
        assert!(
            robust_cv(&durations) < 0.05,
            "cycle length varies: cv {}",
            robust_cv(&durations)
        );
        assert!(
            robust_cv(&volumes) < 0.05,
            "volume varies: cv {}",
            robust_cv(&volumes)
        );
        let duty = median(&duties, 0.0);
        assert!((duty - 0.5).abs() < 0.06, "duty {duty}");
    }

    #[test]
    fn breath_rate_matches_the_generator() {
        for period in [3.0_f32, 4.0, 5.0, 6.0] {
            let minutes = 6.0;
            let signal = analyze(
                &sine(minutes as usize, period, 22.0),
                TEST_HZ,
                &[],
                &[],
                TEST_HZ * 30,
            );
            let found = breaths(&signal);
            let expected = 60.0 / period as f64;
            let rate = found.len() as f64 / minutes;
            assert!(
                (rate - expected).abs() < expected * 0.05,
                "period {period}: detected {rate:.1}/min, expected {expected:.1}/min"
            );
        }
    }

    #[test]
    fn amplitude_does_not_change_measured_timing() {
        // Hysteresis proportional to amplitude must not make shallow breathing
        // look irregular. Same period, ten times the amplitude.
        let quiet = analyze(&sine(6, 4.0, 2.5), TEST_HZ, &[], &[], TEST_HZ * 30);
        let loud = analyze(&sine(6, 4.0, 25.0), TEST_HZ, &[], &[], TEST_HZ * 30);
        let quiet_duty = median(
            &breaths(&quiet).iter().map(|b| b.duty).collect::<Vec<_>>(),
            0.0,
        );
        let loud_duty = median(
            &breaths(&loud).iter().map(|b| b.duty).collect::<Vec<_>>(),
            0.0,
        );
        assert!(
            (quiet_duty - loud_duty).abs() < 0.06,
            "{quiet_duty} vs {loud_duty}"
        );
    }
}
