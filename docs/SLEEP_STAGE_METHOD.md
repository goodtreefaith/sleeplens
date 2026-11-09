# SleepLens sleep-stage method (estimator version 4)

This document describes what the estimator computes and how it was checked. It
does not claim accuracy: accuracy is *measured* with the tooling described at the
end, and the measured numbers so far are reported honestly, including where they
are bad.

`docs/SLEEP_STAGE_EVIDENCE.md` covers what the literature supports. This file
covers what the code does.

## Pipeline

```
patient flow (Hz from the card)
  │
  ├─ baseline            moving mean over ±60 s, to remove leak-compensation drift
  ├─ segmentation        breaths between upward zero crossings, with an
  │                      amplitude-relative expiration gate
  ├─ breath features     cycle length, inspiratory time, duty cycle, volume,
  │                      peak flow, mid-inspiratory squareness, flat-top flag
  ├─ windows             90 s (breath detail) and 150 s (slow structure)
  ├─ evidence            per-stage evidence 0..1, converted through a threshold
  ├─ decode              run-length Viterbi over Wake/Light/Deep/REM
  └─ uncertainty         confounded epochs excluded, with a machine-readable reason
```

## Measurement details that matter

Several of these were found by measurement against real recordings, and each one
had been producing silently wrong features before it was fixed. They are recorded
because they are easy to reintroduce.

**Light smoothing only.** An early version low-passed flow with a 100 ms time
constant. That attenuates and phase-shifts the waveform, so breath boundaries land
at a slightly different phase for every breath and *perfectly regular breathing
appears to vary in volume*. It now uses a ~20 ms constant, enough for quantisation
noise only.

**Baseline must be a mean over many breaths.** Per-second medians with a 31-second
window sample the same phase of every breath, which makes the "baseline" a
phase-locked copy of the signal. Per-second **means** over **two minutes** (~30
breaths) are used instead.

**Inspiratory time is measured to the zero crossing, not the peak.** Measuring to
the peak halves it and pushes every epoch's duty cycle below the physiological
range, so every epoch looked flow-limited.

**Expiration depth is relative and capped.** A breath only counts if flow actually
dips into expiration. A fixed depth in litres (the original `0.5`) silently broke
every 8 Hz research channel, whose whole excursion is about 0.2 units: no breath
ever completed and nearly every epoch abstained. The depth is now a fraction of the
breath's peak, clamped to a small multiple of the recent amplitude so one large
artefact cannot raise the bar enough to disable detection for the rest of the night.

**Machine event offsets are seconds; the scan indexes samples.** Comparing them
directly placed every event about fifty times too early at 50 Hz, so the wrong
stretch of night was marked uninterpretable and the recovery breaths that resemble
REM were scored as REM.

## Evidence and the threshold model

For each of Wake, Deep and REM the estimator sums independent cues into a single
non-negative evidence value. That value is converted with

```
score = 2 * evidence / (evidence + threshold) - 1
```

so it is positive only once the evidence exceeds the threshold, and Light is the
zero reference. This is what stops a whole night being labelled Wake or REM merely
because those quantities are non-negative; an earlier version summed unbounded
positive scores and every night decoded to a single class.

| Stage | Cues | Required, not just additive |
| --- | --- | --- |
| Wake | strongest of interval / volume / duty irregularity; fast rate; sighs alongside irregularity; arousal after a machine event; mask-on onset; large breath at a mask edge | wake evidence is discounted when ventilation is far below baseline, which is what keeps REM's irregular low-ventilation breathing from being read as wake |
| Deep | absolute short- and long-window regularity, normal duty cycle and rate, non-flow-limited contour, plus a place in the night's most regular quarter | the night must show real *contrast* in regularity, so a night of unvarying breathing yields no Deep instead of being ranked into it |
| REM | reduced relative ventilation; breath-to-breath irregularity; slow ventilation oscillation; sustained long-window irregularity; spread of per-breath effort | reduced ventilation is a *gate*, because REM and wake share irregularity and only ventilation separates them; flat-topped flow-limitation subtracts |

`Deep.sleep` was removed from the score in v3 and stays out.

## Uncertainty

An epoch is `Uncertain` when the signal cannot support any claim: no usable flow,
artefact or clipping, large leak, periodic (crescendo–decrescendo) breathing, or a
machine event and its recovery. `uncertainRuns` reports the reason and span so the
UI can explain a gap rather than only showing one, and `periodicBreathing` flags a
night where waxing and waning was seen.

The decoder does not vote on uncertainty: those epochs break the chain, so a guess
is never smoothed across uninterpretable time.

## Parameters

All tunable values are in `sleep_stages/params.rs`, grouped by stage. `Params::validate`
rejects incoherent sets. `sleep-eval tune` can perturb them one at a time and keep a
change only if it improves held-out three-class kappa without costing accuracy.

**Nothing in the shipped defaults was fitted.** They were set by hand against
synthetic nights with known stages, then checked against real recordings.

## Measured accuracy

Tooling: `crates/sleeplens-psg` plus the `sleep-eval` binary. It reads real
polysomnography (EDF/EDF+, ucddb `.rec`, Compumedics Profusion XML), scores the
estimator against expert labels, and reports:

* accuracy and Cohen's κ at four, three and two classes;
* per-class recall and precision, because a good κ can hide total failure on Deep;
* **coverage**, because abstaining on hard epochs cannot be allowed to inflate
  accuracy;
* participant-level bootstrap intervals, so one easy recording cannot carry a result;
* the always-Light, majority-class and time-of-night baselines.

### What has been measured so far

Two datasets were available. They point in the same direction and both are far
from good.

**Sleep-EDF** — 32 night pairs, 32 participants, 28,016 scored epochs. Respiration
here is an oro-nasal thermistor at only **1 Hz**, which cannot support breath-level
features, so coverage is low and the four-class numbers should be read as a floor
rather than a fair test of the method.

| Metric | v4 | Always Light | Time of night |
| --- | --- | --- | --- |
| Four-class accuracy | 42.5 % | 19.5 % | 44.4 % |
| Four-class κ | 0.160 | 0.000 | 0.184 |
| Three-class κ | 0.207 | — | — |
| Sleep/wake accuracy | 62.3 % | — | — |
| Sleep/wake κ | 0.243 | — | — |
| Coverage | 45.0 % | 100 % | 100 % |

Participant-bootstrap 95 % intervals for the four-class kappa are 0.062 to 0.236,
and for accuracy 35.1 % to 49.6 %. The kappa interval still includes the
time-of-night baseline's point estimate, so with 32 participants this data cannot
show that the breathing features beat a time-of-night prior even where they
slightly do.

**ucddb** — 4 truncated night files, 4 participants, 422 scored epochs. This has a
true 8 Hz flow channel but very little data.

| Metric | v4 | Always Light | Time of night |
| --- | --- | --- | --- |
| Four-class accuracy | 34.8 % | 38.3 % | 37.6 % |
| Four-class κ | 0.062 | 0.000 | 0.054 |
| Sleep/wake κ | 0.252 | — | — |
| Coverage | 71.9 % | 100 % | 100 % |

Per-class recall on Sleep-EDF: Wake 67.6 %, Light 40.0 %, Deep **0.0 %**, REM
25.6 %. Deep is never predicted on either dataset, which is consistent with the
literature's finding that N3 is the hardest stage to recover from respiration and
with the estimator's deliberately conservative Deep gate.

**Read this as a negative result.** On Sleep-EDF the estimator is beaten outright by
a time-of-night prior on four-class agreement (42.5 % vs 44.4 %), and only sleep
versus wake clearly beats chance. On ucddb four-class agreement is *below*
always-Light. The method is not yet a useful four-class stager, and the app's
wording must not imply otherwise.

Caveats:

1. **The 1 Hz channel is the wrong instrument.** Sleep-EDF's thermistor cannot
   resolve inspiratory duty cycle or breath-to-breath shape, which is what most of
   the model's evidence depends on. Its numbers understate what an 8 Hz or 50 Hz
   channel can support — but they are also the only 32-participant measurement
   available, so they are reported rather than explained away.
2. **ucddb has four nights**, two of them truncated by an interrupted download, and
   its interval for κ spans −0.03 to 0.15. It cannot distinguish this from several
   other parameter settings.
3. **Neither channel is PAP flow.** A thermistor and a lab flow sensor are not a mask
   circuit; amplitude and shape do not transfer directly.
4. **Low coverage.** 42–72 % of labelled epochs are abstentions, and the interval on
   coverage is wide. Abstaining on hard epochs is the honest behaviour, but it means
   the headline accuracy describes only the easier part of each night.

The measurements taken during development are why the estimator looks the way it
does: an earlier revision scored *worse* than its predecessor (four-class κ 0.029 vs
0.097 on ucddb) after a round of hand-tuning against synthetic data. Several genuine
bugs were found this way — the event-offset unit error, the fixed expiration floor,
and the duty-cycle measurement error — and the current defaults were chosen by
measuring alternatives on real data rather than by intuition.

### Not yet done

* **Paired PAP-flow and PSG recordings.** This is the only way to know how the
  estimator performs on an AS100 night, and it remains the blocking item.
* **A cohort with a breath-resolving airflow channel.** ucddb has the right sensor
  and too few nights; Sleep-EDF has the nights and the wrong sensor. MESA nasal
  pressure at 32 Hz is the closest public candidate (`--layout nsrr`,
  `--respiration-channel nasal`) and needs the reviewed NSRR agreement.
* Tuning rather than hand-setting the absolute caps. `sleep-eval tune` exists for
  this but four nights cannot support it.

## Reproducing

```bash
# Measure on any supported layout.
cargo run --release -p sleeplens-psg --bin sleep-eval -- run  <dir> --layout sleep-edf
cargo run --release -p sleeplens-psg --bin sleep-eval -- run  <dir> --layout ucddb
cargo run --release -p sleeplens-psg --bin sleep-eval -- run  <dir> --layout nsrr

# One night's hypnogram next to the estimate.
cargo run --release -p sleeplens-psg --bin sleep-eval -- night <dir> --layout ucddb --only ucddb002

# Held-out parameter search, keeping repeat nights of one participant together.
cargo run --release -p sleeplens-psg --bin sleep-eval -- tune <dir> --layout nsrr --out tuned.txt

# Synthetic behaviour, for checking a change did what it intended.
cargo run --release -p sleeplens-core --example synthetic
```

Datasets used during development: [ucddb](https://physionet.org/content/ucddb/1.0.0/)
(PhysioNet, open), [Sleep-EDF](https://physionet.org/content/sleep-edfx/1.0.0/)
(PhysioNet, open; respiration is 1 Hz, so only sleep/wake is testable from it), and
MESA/SHHS via NSRR (reviewed data-use agreement required).
