# SleepLens sleep-stage method and evaluation

SleepLens uses an experimental breathing-based heuristic. Its results are not a
sleep study, and measurements on the public datasets below do not establish
accuracy on an Aeonmed AS100 night. [The literature review](SLEEP_STAGE_EVIDENCE.md)
explains which respiratory features have supporting evidence.

Estimator version 5 includes the bounded contour correction described below.
It improves Deep recall on UCDDB without reducing accuracy or kappa on either
complete corpus or their reserved-participant subsets. Sleep-EDF Deep recall
remains zero. No fitted parameter changes were accepted.

## Signal and scoring

The estimator works on 30-second epochs:

1. Subtract a moving mean over per-second flow means, with 60 seconds of context
   on either side. This removes slow offset drift without tracking individual
   breaths.
2. Detect breaths between upward baseline crossings. Expiration must reach a
   depth relative to the recent amplitude before another breath can begin.
3. Measure cycle length, inspiratory duration, inspired volume, duty cycle,
   peak flow and mid-inspiratory squareness.
4. Pool breath features over up to 90 seconds and 300 seconds. Windows stop at
   missing or unusable neighboring epochs. Earlier documentation incorrectly
   called the long window 150 seconds; the implemented `window(..., 4, 5)` spans
   ten 30-second epochs.
5. Score Wake, Deep and REM evidence against a zero-score Light reference.
   Evidence becomes positive only after exceeding the corresponding threshold.
   Deep needs absolute regularity, an eligible place in the night's regular
   quartile, duty/rate checks and a contour check. REM needs reduced relative
   ventilation and irregularity. These rules are heuristics, not stage identifiers.
6. Decode the sequence with bout-length transition costs. No fixed 90-minute
   cycle is imposed.

An observed epoch becomes `Uncertain` when its flow cannot support a claim,
including artifact, large leak, a breathing event and recovery, or detected
periodic breathing. Missing flow remains `NoData`. Uncertain epochs break the
sequence rather than allowing the decoder to smooth a guess across them.

`signalQuality` describes usable flow. The fixed confidence labels are not
calibrated probabilities or accuracy measurements.

## Evaluation corrections

The current evaluation uses the existing `sleeplens-psg` reader, `sleep-eval`
commands and metrics. It does not replace them with a new evaluator.

Several errors in the earlier measurements required correction:

- Sleep-EDF repeat nights belong to one person. `SC4001E0` and `SC4002E0` both
  belong to `SC400`, rather than two participants. Bootstrap intervals and splits
  now use that participant identity.
- The cassette manifest contains 153 recordings: 102 with `E0`, 31 with `F0` and
  20 with `G0` suffixes. The old reader accepted only `E0` and only `EC`/`EH`
  hypnogram scorer letters. Pairing now uses the shared recording prefix and
  accepts the dataset's scorer letters.
- UCDDB codes differ from Profusion codes. Its official legend is 0 Wake,
  1 REM, 2 Stage 1, 3 Stage 2, 4 Stage 3, 5 Stage 4, and 6/7 excluded.
  Light combines 2/3 and Deep combines 4/5. Earlier UCDDB results used the wrong
  mapping and are withdrawn. See the
  [official annotation legend](https://physionet.org/content/ucddb/1.0.0/).
- The evaluation reader rejects a recording shorter than its header-declared
  byte count. The download scripts publish a final filename only after exact
  size and official SHA-256 verification. Partial files are not complete nights.
- The majority baseline now finds the evaluated cohort's most common label.
  It previously duplicated always-Light. Ties follow Wake, Light, Deep, REM.

The historical report called 25 Sleep-EDF recordings 25 participants, even
though repeat nights share participants. Its truncated UCDDB files and wrong
label mapping also prevent a valid comparison. Those small-sample figures are
not a valid before/after comparison with this larger, verified corpus. The
comparisons below run original estimator v4 from commit `ccaba79` and v5 through
the same corrected reader and reporting code, on the same frozen files.

## Data and metric definitions

[UCDDB](https://physionet.org/content/ucddb/1.0.0/) is complete: 25 recordings,
25 expert stage files and 25 participants, totaling 805,923,999 bytes. All 50
files passed independent checks against the official sizes and SHA-256 values.
Its `Flow` channel is sampled at 8 Hz and labeled L/min, but the dataset describes
an oro-nasal thermistor, not PAP patient flow.

The full [Sleep-EDF cassette corpus](https://physionet.org/content/sleep-edfx/1.0.0/)
is complete: 153 recordings from 78 people and 153 hypnograms, totaling
7,596,565,380 bytes. All 306 files passed independent official size and SHA-256
checks. Its respiration channel is a 1 Hz oro-nasal thermistor.

The reader resamples research respiration to 50 Hz and normalizes amplitude.
Resampling adds no sensor information, and normalization does not turn a
thermistor into calibrated PAP flow. The PSG harness supplies no excess-leak
channel or machine-scored event offsets, so it does not validate those parts of
the app's CPAP input path.

Every report includes two views:

- The coverage view scores only claimed stages. Read its accuracy together with
  coverage. Pooled coverage weights epochs; mean per-night coverage weights each
  night equally.
- The closed-set view forces abstentions to Light for a sensitivity analysis.
  It is not the app's displayed output: the app retains Uncertain. This view
  includes all valid labeled epochs in the recording.

Four classes are Wake, Light, Deep and REM. Three classes combine Light and Deep
into NREM. Binary results combine Light, Deep and REM into Sleep. Artifact and
unscored reference labels are excluded from both views.

Always-Light, cohort-majority and time-of-night baselines cover all valid labels.
Compare them with the closed-set view, rather than a model accuracy measured on
only its claimed subset. The cohort-majority reference uses the evaluated labels;
it is descriptive, not a predictor fitted on training data.

The `TimeOnly` baseline predicts Wake for the first quarter of the recording and
Light afterward. It uses normalized recording position, not clock time or a
measured sleep cycle. The harness prints it as `time of night only`; that label
does not make it a physiological model, especially for a 20-hour Sleep-EDF
recording.

Confidence intervals use 2,000 participant-bootstrap resamples with seed
`0x5EED`. Repeat nights stay together. They describe cohort sampling uncertainty,
not the confidence of an individual predicted stage.

## Complete UCDDB results

Both versions use 19,535 claimed epochs and 1,239 abstentions, from 20,774 valid
reference epochs. Pooled coverage is 94.0%; mean per-night coverage is 93.9%.
Neither changes with the contour correction.

| View | Classes | v4 accuracy | v5 accuracy | v4 kappa | v5 kappa |
| --- | --- | --- | --- | --- | --- |
| Claimed epochs | Four | 38.869% | 39.688% | 0.059633 | 0.082505 |
| Claimed epochs | Three | 49.086% | 49.286% | 0.114020 | 0.116023 |
| Claimed epochs | Sleep/wake | 72.598% | 72.598% | 0.154027 | 0.154027 |
| Abstentions forced to Light | Four | 39.131% | 39.901% | 0.051431 | 0.073352 |
| Abstentions forced to Light | Three | 49.013% | 49.201% | 0.098812 | 0.100725 |
| Abstentions forced to Light | Sleep/wake | 71.883% | 71.883% | 0.132979 | 0.132979 |

Four-class macro F1 on claimed epochs changes from 0.281779 to 0.324269.
Mean per-night accuracy changes from 41.3% to 42.0%; the table above reports
pooled accuracy.

| Deep reference population | Epochs | v4 recall | v5 recall | v4 precision | v5 precision |
| --- | --- | --- | --- | --- | --- |
| Claimed epochs | 2,606 | 1.9% | 12.7% | 29.1% | 55.9% |
| All valid epochs | 2,663 | 1.9% | 12.4% | 29.1% | 55.9% |

The four-class participant-bootstrap 95% accuracy interval is 35.4% to 42.5%
for v4 and 36.0% to 43.6% for v5. The corresponding kappa intervals
are 0.019 to 0.097 and 0.037 to 0.125. These intervals overlap; an interval for
the paired gain was not computed.

The full-coverage baselines are always-Light at 50.0% accuracy, cohort-majority
at 50.0%, and time-of-night at 48.9% with kappa 0.102. The v5 closed-set
four-class accuracy of 39.901% remains below these baselines. This is not yet a
useful four-stage stager.

### Reserved-participant check

The existing `compare` command reserves the final quarter of sorted participant
IDs. Here that is seven people, `ucddb022` through `ucddb028`, with 5,552 claimed
epochs and 163 abstentions. The initial diagnostic recording, `ucddb002`, is
outside this subset.

| View | Classes | v4 accuracy | v5 accuracy | v4 kappa | v5 kappa |
| --- | --- | --- | --- | --- | --- |
| Claimed epochs | Four | 34.744% | 34.852% | 0.049219 | 0.055500 |
| Claimed epochs | Three | 40.400% | 40.400% | 0.080347 | 0.080347 |
| Claimed epochs | Sleep/wake | 65.868% | 65.868% | 0.170334 | 0.170334 |
| Abstentions forced to Light | Four | 35.363% | 35.468% | 0.049048 | 0.055253 |
| Abstentions forced to Light | Three | 40.910% | 40.910% | 0.077507 | 0.077507 |
| Abstentions forced to Light | Sleep/wake | 66.002% | 66.002% | 0.164892 | 0.164892 |

Pooled coverage is 97.1%; mean per-night coverage is 97.0%. Deep recall on the
569 claimed Deep epochs changes from 0% to 3.9%, with 47.8% precision. Over all
572 valid Deep epochs it is 3.8%. Three-class and sleep/wake results are unchanged
on this subset. The v5 four-class claimed-epoch 95% accuracy interval is 31.0%
to 38.3%; the kappa interval is 0.011 to 0.096. This check is separate from the
tuning split below.

## Complete Sleep-EDF results

Both versions use 205,827 claimed epochs and 209,134 abstentions, from 414,961
valid reference epochs. Pooled coverage is 49.6%; mean per-night coverage is
49.9%. Every confusion matrix is identical between v4 and v5.

| View | Classes | v4 and v5 accuracy | v4 and v5 kappa |
| --- | --- | --- | --- |
| Claimed epochs | Four | 50.686% | 0.169984 |
| Claimed epochs | Three | 52.645% | 0.188701 |
| Claimed epochs | Sleep/wake | 61.593% | 0.239668 |
| Abstentions forced to Light | Four | 29.124% | -0.108498 |
| Abstentions forced to Light | Three | 30.708% | -0.116965 |
| Abstentions forced to Light | Sleep/wake | 36.151% | -0.129125 |

Four-class macro F1 on claimed epochs is 0.304104. Mean per-night accuracy is
46.8%. The four-class participant-bootstrap 95% accuracy interval is 46.9% to
54.1%; the kappa interval is 0.133 to 0.202. Both versions have 0% Deep recall
and 0% precision, over 10,499 claimed Deep epochs and 13,039 valid Deep epochs.

The full-coverage baselines are always-Light at 21.8% accuracy, cohort-majority
Wake at 68.8%, and time-of-night at 46.3% with kappa 0.190. The model's closed-set
four-class accuracy is higher than always-Light but lower than the other two
references. Its claimed-epoch accuracy uses a smaller denominator and does not
establish an improvement over those full-coverage references.

### Reserved-participant check

The same final-quarter rule reserves 20 people and their 40 recordings:
`SC459`, `SC460`, `SC461`, `SC462`, `SC463`, `SC464`, `SC465`, `SC466`, `SC467`,
`SC470`, `SC471`, `SC472`, `SC473`, `SC474`, `SC475`, `SC476`, `SC477`, `SC480`,
`SC481`, `SC482`. There are 77,987 claimed epochs and 28,792 abstentions, from
106,779 valid epochs. Pooled coverage is 73.0%; mean per-night coverage is 73.2%.
Every confusion matrix is again identical between versions.

| View | Classes | v4 and v5 accuracy | v4 and v5 kappa |
| --- | --- | --- | --- |
| Claimed epochs | Four | 62.074% | 0.073945 |
| Claimed epochs | Three | 62.313% | 0.074429 |
| Claimed epochs | Sleep/wake | 65.113% | 0.099371 |
| Abstentions forced to Light | Four | 51.159% | 0.016505 |
| Abstentions forced to Light | Three | 51.931% | 0.017133 |
| Abstentions forced to Light | Sleep/wake | 55.296% | 0.026483 |

Mean per-night accuracy is 61.7%. The four-class claimed-epoch 95% accuracy
interval is 60.2% to 64.0%; the kappa interval is 0.035 to 0.120. Deep recall and
precision remain zero, over 1,654 claimed and 2,292 valid Deep epochs.
These reserved-subset checks test a structural correction diagnosed elsewhere.
They are not an independent clinical validation or a confidence interval for its
gain. The tuning experiment below has its own separate participant split.

## Why the Deep gate was changed

The claim that v4 never predicts Deep did not survive complete, correctly labeled
data. Its full UCDDB recall is 1.9%. A separate contour bug was still present.

Squareness is the mean flow over the middle of inspiration divided by peak flow.
A value near one is a flatter contour. The existing flow-limitation feature starts
increasing at 0.95. The old Deep gate instead required squareness of at least
0.90 and rewarded larger values, favoring the direction described as flattening.
Version 5 uses a maximum of 0.95 and reverses that reward. Other variability,
rate, duty, rank and contrast thresholds remain unchanged.

The real-data feature example uses the same measurement windows and exclusions
as the estimator. Medians over the complete UCDDB reference classes are:

| Feature | Deep, 2,663 epochs | Light, 10,388 epochs |
| --- | --- | --- |
| Short-window interval CV | 0.06229 | 0.09298 |
| Long-window interval CV | 0.06870 | 0.10540 |
| Short-window volume CV | 0.11077 | 0.17098 |
| Long-window volume CV | 0.13166 | 0.23176 |
| Short-window squareness | 0.89481 | 0.89791 |

CV here is the robust spread `IQR / (1.349 * median)`. The distributions include
the stated reference classes; the example also prints uncertainty counts. Deep
has 57 abstentions and Light has 536.

Among 1,478 supported regular-quartile Deep epochs, the original contour condition
rejects 784. The short-volume cap rejects 436, the long-volume cap rejects 213,
and the long-ventilation cap rejects 109. These failures overlap. The corrected
maximum-contour condition rejects 206. This supports a bounded correction to the
inverted contour check, not loosening all caps until Deep appears.

The 0.95 boundary comes from the existing flattening feature, not a search over
PSG labels or the synthetic generator. No CV cap was fitted. Changes in Deep
evidence can still affect other decoded stages through transition costs, which
is why the reports include every class and granularity.

The Sleep-EDF feature diagnosis covers only recording `SC4001E0`, not all 153
recordings. It contains 220 Deep epochs and 308 Light epochs. Their median
short-window interval CVs are 0.06633 and 0.08432; long-window interval CVs are
0.07060 and 0.09124. Short-window volume CVs are 0.31036 and 0.47935;
long-window volume CVs are 0.37074 and 0.51488. Among 130 supported
regular-quartile Deep epochs, the short-volume cap rejects 125, the long-volume
cap rejects 93 and the long-ventilation cap rejects 46. Failures overlap. The
corrected contour cap rejects none of these epochs, yet none exceeds the Deep
evidence threshold. This identifies volume gates in this recording; it does not
prove one cause for the full corpus. Those caps remain unchanged.

## Parameter search

The existing `tune` command now separates training, selection and final test
participants. It no longer accepts a one-percentage-point accuracy loss. A
candidate must improve four-class kappa without losing accuracy, and the other
reported granularities and closed-set views must not regress. Rejected candidates
fall back to the default preset; the command does not edit app defaults.

This run starts from the unfitted contour preset, rather than changing the v4
baseline used in the earlier tables. The complete UCDDB split uses 15 training,
five selection and five final test participants. IDs are sorted before splitting:

- Training: `ucddb002`, `ucddb003`, `ucddb005`, `ucddb006`, `ucddb007`,
  `ucddb008`, `ucddb009`, `ucddb010`, `ucddb011`, `ucddb012`, `ucddb013`,
  `ucddb014`, `ucddb015`, `ucddb017`, `ucddb018`.
- Selection: `ucddb019`, `ucddb020`, `ucddb021`, `ucddb022`, `ucddb023`.
- Final test: `ucddb024`, `ucddb025`, `ucddb026`, `ucddb027`, `ucddb028`.

The search tries the bounded single-step adjustments in `candidate_adjustments`.
Multipliers apply sequentially to the current selected candidate, not as a
continuous fit over the allowed bounds. Only two adjustments survived selection:

| Parameter | Default | Trial multipliers | Guard bounds | Selected value |
| --- | --- | --- | --- | --- |
| `emission_scale` | 4.0 | 1.3 and 0.8 | 0.2 to 10.0, after validation | 5.2 |
| `rem_irregular` | 0.34 | 1.25 and 0.8 | 0.1 to 3.0 | 0.425 |

On the five final test people, with 3,929 claimed epochs and 138 abstentions,
four-class accuracy falls from 37.567% to 36.651% and kappa falls from 0.073438
to 0.068340. The fitted candidate is rejected. Neither selected value is adopted.
The remaining parameter groups retain their defaults too.

The default preset's final-test 95% kappa interval is 0.040 to 0.115; the fitted
candidate's interval is 0.031 to 0.117. Their accuracy intervals are 35.1% to
39.6% and 34.1% to 39.0%. These are participant-bootstrap metric intervals, not
uncertainty intervals for fitted coefficients. No parameters were
fitted against synthetic data. Synthetic tests only check intended behavior.

## What remains unmeasured

Neither dataset contains paired AS100 PAP-flow and expert PSG stages. Accuracy
on that device is unknown. The 1 Hz Sleep-EDF signal cannot validate fine
breath-shape measurements, and the 8 Hz UCDDB thermistor still differs from PAP
flow. Neither sensor establishes an upper or lower performance bound for AS100.

NSRR MESA/SHHS was not evaluated. Approved access and the reviewed agreement were
not available in this session. For an authorized NSRR dataset, the existing
reader supports explicit channel selection with `--respiration-channel nasal`.
A nasal-pressure channel does not remove the need for paired PAP validation.

## Reproduce the measurements

Keep real recordings outside the repository. The [public evaluation record](evaluation/2026-11-09/README.md)
contains the exact final reports, official input hashes and source provenance.
It contains no recordings or local paths. External frozen pair lists and
independent checksum audits identify the corresponding downloaded files.
Both benchmark executables must use the corrected PSG reader; checking out the
old evaluator would restore its label and participant errors.

```bash
# Full reports, including both views and baselines.
cargo run --release -p sleeplens-psg --bin sleep-eval -- run /path/to/sleep-edfx --layout sleep-edf
cargo run --release -p sleeplens-psg --bin sleep-eval -- run /path/to/ucddb --layout ucddb

# A night's labels and estimate, followed by existing gate diagnostics.
cargo run --release -p sleeplens-psg --bin sleep-eval -- night /path/to/ucddb --layout ucddb --only ucddb002 --trace

# Real Deep/Light short- and long-window CVs, using the same PSG reader.
cargo run --release -p sleeplens-psg --example stage-features -- /path/to/ucddb ucddb

# Participant-separated parameter search. This writes a report, not defaults.
cargo run --release -p sleeplens-psg --bin sleep-eval -- tune /path/to/ucddb --layout ucddb --out /path/to/tuning.txt

# Behavior check only, never a fitting target or an accuracy benchmark.
cargo run --release -p sleeplens-core --example synthetic
```
