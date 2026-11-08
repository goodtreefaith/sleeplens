# Card format notes

Working notes from inspecting real cards. Serial numbers are written as
`<SERIAL>`; never paste real serials, raw files or real dates here.

Confidence labels: **Confirmed** (checked against independent data on the
same night), **Likely** (strong statistical evidence), **Guess**.

## Aeonmed AS100 series (internal model string `AS2X`, firmware `AM-1.01..47.1`)

Sold to patients as the **Aeonmed AS100 series** auto CPAP. Internally the
device identifies itself as `AS2X` (in the `.dat` header and the `.bkp`
log), so the parser and code use `as2x`. If you own another Aeonmed model,
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
689 consecutive nights (2024-2026): identical.

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
missing data. Usage time = last timestamp − first timestamp (the device's
own usage figure matches this span rounded up), not the record count.

#### Sample slots (index into the 400 u16 values)

Slots 20-369 are 50 Hz waveforms (50 samples = 1 s), continuous across
records. Breathing shows up at ~0.3 Hz in every breathing channel.

| Slots | Rate | Meaning | Scale | Confidence |
| --- | --- | --- | --- | --- |
| 0-9, 10-19 | 10 Hz × 2 | Raw sensor counts (two sensors) | raw ADC | Likely |
| 20-69 | 50 Hz | **Set (target) pressure** | ÷10 → cmH2O | Confirmed (matches summary mean and min/max settings) |
| 70-119 | 50 Hz | Measured pressure, blower side | ÷10 → cmH2O | Likely |
| 120-169 | 50 Hz | Measured pressure, mask side | ÷10 → cmH2O | Likely |
| 170-219 | 50 Hz | Breath-synced 0-130 signal: snore or vibration | ? | Guess |
| 220-269 | 50 Hz | **Patient flow** | (v − 500) → L/min | Likely (gives 0.38 L tidal volume, 19 breaths/min) |
| 270-319 | 50 Hz | Stepped 0-32 index: flow limitation? | ? | Guess |
| 320-369 | 50 Hz | Total flow (patient + leak) | (v − 500) → L/min | Likely |
| 370-399 | 1 Hz | Per-second status values, below | | |

Per-second status slots:

| Slot | Meaning | Confidence |
| --- | --- | --- |
| 374 | Constant 25 | |
| 375 | 293-319, follows pressure: temperature ×10 °C? | Guess |
| 376 | ~56: humidity %? | Guess |
| 379 | Elapsed seconds counter | Likely |
| 380 | Constant 50 (waveform sample rate) | Likely |
| 381 | Small state code (5, 11, 13, 15, 27, 28, 52): algorithm state | Guess |
| 383 | 21-25: ambient temperature °C? | Guess |
| 384 | **Leak, L/min** (correlates 0.90 with total minus patient flow) | Likely |
| 391 | 4000-8000: blower speed? | Guess |
| 370, 371, 373, 377 | Slowly varying raw values (sensor references?) | Guess |
| 372 | Signed i16 | Guess |
| others | Always zero so far | |

### `.bkp` log

Made of 128-byte blocks. The first bytes give the block type:

| First bytes | Block | Count seen |
| --- | --- | --- |
| `C5 C5` | Settings snapshot | ~420 |
| `A5 A5` | **Session summary** (one per session) | 384 (ring buffer, ~1 year) |
| year u16 (`EA 07` = 2026) | Text event line | ~1000 |
| 2-digit year byte (`19`, `1A`) | 16 × 8-byte therapy event entries | ~15,000 entries |
| `FF FF` | Empty | |
| year u16 without `FF` at byte 7 | Unknown (~66 blocks) | |

The log is a ring buffer: on the sample card, session summaries cover the
last ~12 months and event entries the last ~14 months. **Older nights have
waveforms in `.dat` but no machine-scored AHI.**

#### `A5 A5` session summary (checked on two nights)

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
| 0x50 | 2 | A high pressure percentile ×10; stored 7.5 where per-second p95 is 7.8, so the exact definition is unknown (Guess) |
| 0x52 | 2 | 0 |
| 0x54 | 2 | **Usage minutes** (Confirmed: 504 for a 23:13:58-07:37:13 night) |
| 0x56 | 2 | 0 |
| 0x58 | 2 | **AHI ×10** (Confirmed: 13 = 11 logged events / 8.4 h) |
| 0x5A | 2 | Apnea index ×10 (Likely) |
| 0x5C | 2 | Hypopnea index ×10 (Likely) |
| 0x5E | 2 | Max pressure setting ×10 |
| 0x60 | 2 | Unknown small value (4-5) |

#### 8-byte therapy event entries (Confirmed)

```
[yy] [ss] [MM] [dd] [code] [hh] [mi] [value]
```

`yy` is year − 2000. Codes:

| Code | Meaning | `value` | Confidence |
| --- | --- | --- | --- |
| `E0` | Pressure change | new pressure in 0.5 cmH2O steps (10 → 5.0) | Likely |
| `E1` | Apnea, type 1 | pressure at event (0.5 steps) | Confirmed apnea; type unknown |
| `E2` | Apnea, type 2 | pressure at event | Confirmed apnea; type unknown |
| `E3` | Hypopnea | value (0.5 steps) | Confirmed |
| `E8` | Level 0-6 that rises and falls in runs: snore or flow-limitation level | level | Guess |
| `EE`, `E7`, `EF` | Session start/end markers | | Likely |

Events are stamped at the **end** of the event: flow amplitude drops below
~15% of normal for 10-20 s and recovers right after the stamp. E1 + E2 per
night match the apnea index, E3 matches the hypopnea index. Until E1 versus
E2 is pinned down (obstructive vs clear airway), show them together as
"apneas".

#### `C5 C5` settings snapshot

Starts `C5 C5 01`, then u16 values; on the sample card `0x32` (5.0 min
pressure), `0x3C`, `0x32`, `0x50` (8.0 max pressure). Timestamp (year u16,
month, day, hour, minute, second) at offset `0x21`. Humidifier level near
offset `0x2E`. Field map still open.

#### Text event lines

Year u16, month, day, hour, minute, second, `FF`, then ASCII up to a NUL, for
example `Stop work by Auto Off in WorkScreen`, `Parameter change Humidifire
15 to 2`, `SD card Insert`, `Machine Reset = 6  Software Version …`.
Useful for mask on/off, humidifier changes and resets.

### Open questions

1. E1 versus E2: obstructive or clear airway?
2. Exact meaning of slots 170-219, 270-319 and code `E8`.
3. Full `C5 C5` field map (ramp, humidity, comfort settings).
4. Leak (slot 384): total leak including the mask vent, or unintentional
   leak only? Median ~29 L/min suggests total leak.

## ResMed AirSense 10 / 11

Public EDF+ format, documented by OSCAR. `STR.edf` has one row per day with
summary statistics (usage, AHI, leak, pressure percentiles); `DATALOG/<date>/`
has per-session EDF files (`BRP` flow/pressure 25 Hz, `PLD` 0.5 Hz, `EVE`
events, `SA2` oximetry). Parsing is mostly straightforward with an EDF reader.
