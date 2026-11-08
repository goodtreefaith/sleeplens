# How SleepLens presents data (and how it differs from OSCAR)

[OSCAR](https://www.sleepfiles.com/OSCAR/) is the standard free tool for CPAP
data and the reference for what power users expect. It does not support the
Aeonmed AS100. SleepLens should show the same truth, but be readable by
someone who has never heard of AHI.

## What OSCAR does

| Area | What it shows |
| --- | --- |
| **Daily** tab | Left sidebar: calendar, machine and settings, a table of event indices (AHI, obstructive, central, hypopnea…), a statistics table (min / median / 95% / max for pressure, leak, breathing rate, tidal volume, minute ventilation, flow limitation, snore) and a session list. Right side: a tall stack of 10-15 time-synced graphs: event flags, flow rate, pressure, leak rate, snore, flow limitation, breathing rate, tidal volume, minute ventilation and more. |
| **Overview** tab | Bar charts per day for a date range: AHI split by event type, usage hours, session times, pressure, leak. |
| **Statistics** tab | A large table with columns for last night, last week, last month, last 6 months and last year: usage, compliance, AHI, leak, pressure percentiles. |
| **Oximetry** tab | Pulse oximeter import, if you have one. |

What OSCAR gets right: every number is available, graphs share one time axis
so you can line up a leak with an event, and percentiles (median, 95th) are
used instead of averages that hide spikes.

## What Aeonmed's own tools show

The AS100 screen shows P90 pressure, AHI, SNI (snore index), leak, treatment
time and average pressure for a chosen period, with up to a year of history.
NeoRead, Aeonmed's Windows app, imports the card's `.bkp` log and night files
and produces printable reports and graphs. Like OSCAR, it is desktop software
for clinicians and dealers, with no plain-language interpretation.

## Where people struggle with OSCAR

1. **No interpretation.** Values come with no "is this good?" Forums are
   full of "please read my OSCAR charts" posts.
2. **Jargon everywhere.** AHI, RERA, FL, CA/OA, 95%, minute ventilation,
   with no inline explanation.
3. **Everything at once.** The daily page opens on a dense wall of graphs
   with small text. The three numbers most people need (hours, AHI, leak) are
   in a sidebar table.
4. **Desktop only.** It is a Qt app you have to install, import into and keep
   updated. It has no phone layout.
5. **Trends take work.** Overview needs a date range and reading several bar
   charts side by side. Nothing says "your leak has been getting worse since
   March".
6. **Unknown leak and event reliability.** Large leaks make event counts
   unreliable, but nothing on screen links the two.

## The SleepLens approach

**Answer first, evidence on demand.** Each screen opens with a sentence, then
the few numbers that back it up, then the charts.

### 1. Night summary (replaces OSCAR's Daily sidebar)

- **Headline sentence**, written from the data, e.g. "8 h 23 m with a good
  seal. 11 breathing pauses, about 1 an hour, which is in the normal range."
- **Four tiles**, each with a value, a status word, a range bar showing where
  the value sits between "good" and "talk to your clinician", and a "What is
  this?" disclosure:
  - Hours used
  - Breathing events per hour (AHI), with a breakdown: apneas, hypopneas
  - Mask seal (leak), described as good/some/large rather than L/min first
  - Pressure: median and high, against the machine's min/max setting
- **Data note** when it matters: "Event counts are less reliable on nights
  with large leaks", "This machine's log only keeps about a year, so AHI is
  not available for this night".

### 2. Night timeline (replaces OSCAR's graph wall)

Three lanes by default, on one shared time axis, readable at phone width:

1. **Events lane**: dots for apneas and hypopneas, sized the same, coloured
   by type, with a tap target and a tooltip in plain words.
2. **Pressure lane**: the machine's auto pressure as a step line with the
   min/max setting as a band, so "pressure went up after events" is visible.
3. **Mask seal lane**: leak as a filled area with a "large leak" threshold
   line.

An "Add more" control reveals the advanced lanes (flow, breathing rate,
tidal volume, snore/flow limitation once decoded). Tapping an event zooms to a
2-minute **breath view**: the raw flow waveform with the pause highlighted and
a sentence explaining it ("Airflow dropped for about 15 seconds, then a
recovery breath").

### 3. Trends (replaces Overview + Statistics)

- **Calendar heatmap** of hours per night (one cell per night, whole history
  at a glance; nights under 4 h are visibly different).
- **Compliance**: "You used it 4 h or more on 92% of nights in the last 30
  days", with the common 70% guideline marked.
- **Small multiples** for AHI, leak and pressure with the same time range,
  each with a typical-range band and one-line trend text ("steady", "rising
  since June").
- A compact **comparison table** (last 7 / 30 / 90 / 365 nights), similar to
  OSCAR's Statistics tab but only the rows patients need, with advanced rows
  behind a toggle.

### 4. Doctor report

One printable page that a clinician can read in 30 seconds: compliance,
averages, worst nights, settings history and the plain-language notes.

## Data honesty rules

- If the machine did not record something for a night, say so; never show 0.
- Label apnea types as "type 1 / type 2" until the format notes confirm
  which is obstructive and which is central.
- Thresholds are general guides with sources, not targets, and never advice
  to change settings.
