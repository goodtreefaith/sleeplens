# Evaluation record, 2026-11-09

These are outputs of the existing `sleep-eval` evaluator on complete public
research datasets. See [the method and interpretation](../../SLEEP_STAGE_METHOD.md)
for sensor limits, coverage, the contour correction and rejected tuning.
No recordings, patient examples, credentials or local filesystem paths are
included here. Dataset identifiers are the public research identifiers.

| Dataset | Recordings | Participants | Reference files |
| --- | --- | --- | --- |
| Sleep-EDF cassette | 153 | 78 | 153 hypnograms |
| UCDDB | 25 | 25 | 25 stage files |

Both cohorts passed independent exact-size and official SHA-256 checks. All
153 Sleep-EDF PSG/hypnogram pairs have matching EDF header start dates and times.
The [input manifest](inputs.json) lists official names, sizes, checksums and
source URLs. Retrieve recordings from PhysioNet under its dataset terms.

For each cohort, `*-v4.txt` and `*-v5.txt` are full-corpus `run` reports.
The corresponding `*-heldout.txt` files are the existing `compare` command's
final-quarter participant subset. Reports contain both claimed-epoch and
forced-Light sensitivity views, exact integer confusion matrices, bootstrap
intervals and full-coverage references. See each report for the participant
counts and held-out IDs.

The baseline uses original core source from commit `ccaba79`. Both binaries
use the same corrected PSG reader and report code. The v5 binary uses the final
uncommitted source, including estimator version 5. The
[provenance manifest](provenance.json) identifies source files, binaries and
reports by SHA-256. The working-tree hashes, rather than an uncreated commit,
identify this result. Exact integer matrix checks confirm no accuracy or kappa
regression in all 24 dataset, subset and metric-view comparisons.

[UCDDB tuning](ucddb-tuning.txt) records the participant-separated training,
selection and final-test experiment. The selected fit failed the final test
and was rejected. The shipped values retain the unfitted v5 preset.

These results do not validate AS100 PAP sleep stages. Sleep-EDF uses a 1 Hz
oro-nasal thermistor and UCDDB describes an 8 Hz oro-nasal thermistor signal.
NSRR and paired AS100 PAP-flow/PSG data were not evaluated.
