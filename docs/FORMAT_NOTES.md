# Card format notes

Working notes from inspecting real cards. Serial numbers are written as
`<SERIAL>`; never paste real serials, raw files or real dates here.

Confidence labels: **Confirmed** (checked against independent data on the
same night), **Likely** (strong statistical evidence), **Guess**.

## Aeonmed AS100 series (internal model string `AS2X`, firmware `AM-1.01..47.1`)

Sold to patients as the **Aeonmed AS100 series** auto CPAP. Internally the
device identifies itself as `AS2X` (in the `.dat` header and the `.bkp`
log), so the parser and code use `as2x`.

**Vendor facts** (AS100C/AS100A user manual and vendor pages):

- The device screen shows **P90, AHI, SNI (snore index), leak, treatment
  time and average pressure**, and keeps **1 year of history**. This matches
  the ~1-year ring buffer of session summaries in the `.bkp`.
- The machine distinguishes **central and obstructive apneas**, which fits the
  two apnea codes `E1` and `E2`.
- Settings: humidifier level 0-5, ramp 0-60 min (default 30), Auto ON
  (default on), Auto OFF (default off), "AS-Elex" exhale comfort level 0-3,
  pressure unit hPa (default) or cmH2O.
- Aeonmed's own Windows app, **NeoRead**, imports the card by selecting the
  `.bkp` file ("download from SD") and can load the night files for detailed
  graphs. NeoRead reports are the reference for checking SleepLens numbers. If you own another Aeonmed model,
please open an issue with the folder and file names (not the files) so we
can check whether it shares this format.

All integers are little-endian. Timestamps are local machine time with no
time zone.

### Card layout

```
/<SERIAL><YYYYMM>/            one folder per month (SERIAL = 5 digits)
    <YYYYMMDD>.dat            one file per night, ~18-32 MB
/<XXX><SERIAL>.bkp            4 MB log: settings, session summaries, events, text
```

The `.bkp` name is a 3-letter prefix plus the same 5-digit serial; detect the
card by the month folders and match the `.bkp` whose name ends with that
serial.

The day in the file name is the evening the night started. Layout checked on
all 689 nights on the sample card (2024-2026, with gaps): identical.

### `.dat` night file

| Offset | Size | Meaning | Confidence |
| --- | --- | --- | --- |
| `0x000` | 7 | File created: year u16, month, day, hour, minute, second | Likely |
| `0x050` | 16 | ASCII `AS2X<SERIAL>`, zero-padded | Confirmed |
| `0x060..0x13F` | | Header values; includes an ascending u16 table (ramp?) | Guess |
| `0x140` | 128 | Settings block (`C5 C5`, see below) | Confirmed |
| `0x1C0..0x7FF` | | `0xFF` padding | Confirmed |
| `0x800` | 812 × n | One record per second of therapy | Confirmed |

`(fileSize - 0x800) % 812 == 0` for every file.

#### 812-byte record

| Offset | Size | Meaning | Confidence |
| --- | --- | --- | --- |
| 0 | 2 | Magic `4D 4D` (`MM`) | Confirmed |
| 2 | 2 | `00 00` | Confirmed (always zero so far) |
| 4 | 7 | Timestamp: year u16, month, day, hour, minute, second | Confirmed |
| 11 | 1 | Weekday, Monday = 0 | Confirmed |
| 12 | 800 | 400 × u16 samples, laid out below | |

Timestamps skip one second about every 341 s (clock drift); this is not
missing data. **Usage time = sum of active intervals**: split the record
stream where timestamps jump by more than a minute (mask off) and add up the
intervals. Last-minus-first overstates usage on nights with pauses (one
night spans 678 min but the machine reports 466).

#### Sample slots (index into the 400 u16 values)

Slots 20-369 are 50 Hz waveforms (50 samples = 1 s), continuous across
records. Breathing shows up at ~0.3 Hz in every breathing channel.

| Slots | Rate | Meaning | Scale | Confidence |
| --- | --- | --- | --- | --- |
| 0-9, 10-19 | 10 Hz × 2 | Raw sensor counts (two sensors) | raw ADC | Likely |
| 20-69 | 50 Hz | **Set (target) pressure** | ÷10 → cmH2O | Confirmed (matches summary mean and min/max settings) |
| 70-119 | 50 Hz | Measured pressure, blower side | ÷10 → cmH2O | Likely |
| 120-169 | 50 Hz | Measured pressure, mask side | ÷10 → cmH2O | Likely |
| 170-219 | 50 Hz | Breath-synced spiky 0-200 signal; correlates 0.66 with signed flow. Not proven to be snore | ? | Unresolved |
| 220-269 | 50 Hz | **Patient flow** | (v − 500) → L/min | Likely (gives 0.38 L tidal volume, 19 breaths/min) |
| 270-319 | 50 Hz | Slowly updated 0-87 score; `E8` ≈ floor(1-min mean ÷ 5). Snore vs flow limitation unknown | ? | Likely (score), Unresolved (meaning) |
| 320-369 | 50 Hz | Total flow (patient + leak) | (v − 500) → L/min | Likely |
| 370-399 | 1 Hz | Per-second status values, below | | |

Per-second status slots:

| Slot | Meaning | Confidence |
| --- | --- | --- |
| 374 | Constant 25 | |
| 375 | 227-343: temperature ×10 °C? | Guess |
| 376 | 13-72: relative humidity %? | Guess |
| 379 | Elapsed seconds counter | Likely |
| 380 | Constant 50 (waveform sample rate) | Likely |
| 381 | 0-150, changes only at apnea events and holds between them. 26-150 at every E1, 5-25 at every E2 (1,215 events) | Confirmed association, meaning Unresolved |
| 383 | 21-25: ambient temperature °C? | Guess |
| 384 | **Leak, L/min** (correlates 0.90 with total minus patient flow) | Likely |
| 391 | 0-10000; tracks 375/373, not pressure: thermal control? | Guess |
| 370, 371, 373, 377 | Slowly varying raw values (sensor references?) | Guess |
| 372 | Signed i16, −2071…2146 | Guess |
| others | Always zero so far | |

### `.bkp` log

Made of 128-byte blocks. The first bytes give the block type:

| First bytes | Block | Count seen |
| --- | --- | --- |
| `C5 C5` | Settings snapshot | ~420 |
| `A5 A5` | **Session summary** (one per session) | 384 (ring buffer, ~1 year) |
| year u16 (`EA 07` = 2026) | Text event line | ~1000 |
| 2-digit year byte (`19`, `1A`) | 16 × 8-byte therapy event entries | ~15,000 entries |
| date at `0x4000-0x5B7F` | Counter block: two 64-byte records | 55 blocks |
| `FF FF` | Empty | |

Validate the whole block structure before choosing its type: event blocks
whose first entry has second = 8 read as u16 year 2073/2074 and look like
text lines (11 such blocks on the sample card).

The log is a ring buffer: on the sample card, session summaries cover the
last ~12 months and event entries the last ~14 months. **Older nights have
waveforms in `.dat` but no machine-scored AHI.**

#### `A5 A5` session summary (index formulas checked on 379 nights)

| Offset | Size | Meaning |
| --- | --- | --- |
| 0 | 2 | `A5 A5` |
| 2 | 2 | `01 01` |
| 4 | 2 | Session number u16 |
| 6 | 7 | Session start: year u16, month, day, hour, minute, second |
| 13 | 1 | Weekday |
| 0x40 | 2 | `B5 B5` |
| 0x42 | 7 | Same start timestamp, then `FF` |
| 0x4A | 2 | Counter u16 |
| 0x4C | 2 | Minimum pressure ×10 |
| 0x4E | 2 | Mean pressure ×10, truncated (Confirmed: 6.7 stored for a computed 6.77) |
| 0x50 | 2 | **P90 pressure** ×10, coarse 0.5 steps (Likely: device screen shows P90; matches per-second P90 rounded to 0.5 on 366/379 nights) |
| 0x52 | 2 | 0 |
| 0x54 | 2 | **Usage minutes** (Confirmed: 504 for a 23:13:58-07:37:13 night) |
| 0x56 | 2 | 0 |
| 0x58 | 2 | **AHI ×10** = round-half-up((E1+E2+E3) × 600 / usage min) (Confirmed, 379/379) |
| 0x5A | 2 | Apnea index ×10, E1+E2 (Confirmed, 379/379) |
| 0x5C | 2 | E2 (type 2 apnea) index ×10 (Confirmed, 379/379) |
| 0x5E | 2 | Observed high/max pressure ×10, not the configured max (Likely: 358/379) |
| 0x60 | 2 | Hypopnea index ×10, E3 (Confirmed, 379/379) |

#### 8-byte therapy event entries (Confirmed)

```
[yy] [ss] [MM] [dd] [code] [hh] [mi] [value]
```

`yy` is year − 2000. Codes:

| Code | Meaning | `value` | Confidence |
| --- | --- | --- | --- |
| `E0` | Pressure change | new pressure in 0.5 cmH2O steps (10 → 5.0) | Likely |
| `E1` | Apnea, type 1, **likely obstructive** | pressure at event (0.5 steps) | Confirmed apnea; type Likely |
| `E2` | Apnea, type 2, **likely central / clear airway** | pressure at event | Confirmed apnea; type Likely |
| `E3` | Hypopnea | value (0.5 steps) | Confirmed |
| `E8` | Level 0-14 derived from slots 270-319 (snore or flow limitation) | level | Likely (derivation), Unresolved (meaning) |
| `EE`, `E7`, `EF` | Session start/end markers | | Likely |

Events are stamped at the **end** of the event: flow amplitude drops below
~15% of normal for 10-20 s and recovers right after the stamp. E1 + E2 per
night match the apnea index, E3 matches the hypopnea index.

E1 versus E2: after E1 the machine raises target pressure in 87% of cases
(median +1.9 cmH2O, n=304); after E2 in 24% (median 0.0, n=911). Auto-CPAPs
raise pressure for obstructive events and hold it for central ones, so E1 is
likely obstructive and E2 central. No airway-test waveform was found to
prove it, so show "likely obstructive / likely central" and keep the raw
codes until checked against NeoRead.

#### `C5 C5` settings snapshot

| Offset | Storage | Meaning | Confidence |
| --- | --- | --- | --- |
| `0x00` | `C5 C5` | Magic | Confirmed |
| `0x03`, `0x07` | u16, always equal | Minimum and start pressure ×10 | Likely |
| `0x05` | u16 | Fixed-mode pressure ×10? | Guess |
| `0x09` | u16 | Maximum pressure ×10 | Likely |
| `0x15` | u16, 3 | Comfort (AS-Elex) level? | Guess |
| `0x17` | u8, 1-5 | **Humidifier level** (13/14 text-log changes match) | Confirmed |
| `0x18` | u8, 15 | Ramp minutes? | Guess |
| `0x21` | 7 | Timestamp | Confirmed |
| `0x28` | u8 | Weekday, Monday = 0 | Confirmed |
| `0x2E` | u8, always 3 | Unknown (not humidifier) | Unresolved |
| `0x7F` | `88` | Constant end byte | Confirmed |

Other bytes are flags or zero; see the analysis notes. Settings never
changed deliberately on the sample card, so several fields cannot be told
apart yet.

#### Text event lines

Year u16, month, day, hour, minute, second, `FF`, then ASCII up to a NUL, for
example `Stop work by Auto Off in WorkScreen`, `Parameter change Humidifire
15 to 2`, `SD card Insert`, `Machine Reset = 6  Software Version …`.
Useful for mask on/off, humidifier changes and resets. The "old value" in
`Parameter change` lines is unreliable.

#### Counter blocks (`0x4000-0x5B7F`)

Two 64-byte records per block: date (u16 year, month, day), u32 counter A,
u32 counter B (cumulative), u32 counter C (= B − 1), end byte `88`. A resets
to 0 with the text line `Screen Filter to Reset`, so A is likely operating
time since the filter was reset. Unit roughly 61.4 s per count (Guess).

### Open questions

1. Confirm E1 = obstructive, E2 = central against NeoRead's labels.
2. Physical meaning of slots 170-219, 270-319 and code `E8` (snore vs flow
   limitation); NeoRead's SNI value for a night would settle the snore part.
3. Remaining `C5 C5` fields (ramp, comfort level, Auto ON/OFF): needs a card
   where settings were changed on purpose.
4. Leak (slot 384): total leak including the mask vent, or unintentional
   leak only? Median ~29 L/min suggests total leak.

## ResMed AirSense 10 / 11

Public EDF+ format, documented by OSCAR. `STR.edf` has one row per day with
summary statistics (usage, AHI, leak, pressure percentiles); `DATALOG/<date>/`
has per-session EDF files (`BRP` flow/pressure 25 Hz, `PLD` 0.5 Hz, `EVE`
events, `SA2` oximetry). Parsing is mostly straightforward with an EDF reader.
