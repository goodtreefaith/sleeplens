# Card format notes

Working notes from inspecting real cards. Serial numbers are written as
`<SERIAL>`; never paste real serials, raw files or real dates here.

Confidence labels:

- **Confirmed**: checked against independent data on the same night.
- **Confirmed (NeoRead)**: Aeonmed's own reader, NeoRead, decodes it this
  way (see "NeoRead cross-check" below).
- **Likely (NeoRead, older firmware)**: NeoRead decodes it this way, but
  NeoRead was built for older firmware than the sample card, so the newer
  firmware may differ.
- **Likely**: strong statistical evidence.
- **Guess**: a hunch, not yet tested.

## Aeonmed AS100 series (internal model string `AS2X`, firmware `AM-1.01..47.1`)

Sold to patients as the **Aeonmed AS100 series** auto CPAP. Internally the
device identifies itself as `AS2X` (in the `.dat` header and the `.bkp`
log), so the parser and code use `as2x`.

**Vendor facts** (AS100C/AS100A user manual and vendor pages):

- The device screen shows **P90, AHI, SNI (snore index), leak, treatment
  time and average pressure**, and keeps **1 year of history**. This matches
  the ~1-year ring buffer of session summaries in the `.bkp`.
- The machine distinguishes **central and obstructive apneas**: `E1` is
  obstructive and `E2` is central (Confirmed (NeoRead), see events below).
- Settings: humidifier level 0-5, ramp 0-60 min (default 30), Auto ON
  (default on), Auto OFF (default off), "AS-Elex" exhale comfort level 0-3,
  pressure unit hPa (default) or cmH2O.
- Aeonmed's own Windows app, **NeoRead**, imports the card by selecting the
  `.bkp` file ("download from SD") and can load the night files for detailed
  graphs. NeoRead reports are the reference for checking SleepLens numbers.

If you own another Aeonmed model, please open an issue with the folder and
file names (not the files) so we can check whether it shares this format.

All integers are little-endian. Timestamps are local machine time with no
time zone.

### NeoRead cross-check

We decompiled NeoRead (the 2022 Windows installer, built for device software
1.01.35) to see how the vendor reads the card. This was for interoperability
only. **No NeoRead code is copied into SleepLens, and SleepLens must never
ship or redistribute it.** We only record the facts here: offsets, scales and
labels.

The sample card's firmware is newer than the one NeoRead was built for.
Where the two could differ (mostly event codes), the notes say so.

### Model

NeoRead picks the model from the first 3 characters of the device ID:

| Prefix | Model | Type |
| --- | --- | --- |
| `AS1` | AS100C | CPAP |
| `AS2` | AS100A | Auto CPAP |
| `AS3` | AS100A | Auto CPAP |
| `AS4` | AS100A-R | Auto CPAP |
| `AS5` | AS100A-RT | Auto CPAP |
| `AS6` | AS100C | CPAP |
| `AS7` | AS100C-R | CPAP |
| `AS8` | AS100C-RT | CPAP |
| `BF1`-`BF6` | BF20A, BF20S, BF25A, BF25S, BF25ST, BF30ST | Bilevel |

The sample card says `AS2X…`, so it is an **AS100A** (auto CPAP).
Confirmed (NeoRead).

Therapy modes (settings `Mode` byte): 0 CPAP, 1 Auto CPAP, 2 AUTOS, 3 S,
4 ST. Modes above 1 are for the bilevel BF models only.

NeoRead's graph ranges per model: pressure 0-20, flow −100 to 100, leak
flow 0-50, snore 0-200.

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
| `0x008` | 1 | **Record format version**: 1 = 512-byte records with magic `<<` (`3C 3C`); 2 = 812-byte records with magic `MM` (`4D 4D`). The sample card is version 2 | Confirmed (NeoRead) |
| `0x040` | 16 | Logo string | Confirmed (NeoRead) |
| `0x050` | 24 | Device ID, ASCII `AS2X<SERIAL>`, zero-padded | Confirmed |
| `0x068` | 1 | Device type u8 | Confirmed (NeoRead) |
| `0x069` | 4 | Software (firmware) version, printed most significant byte first (for example 1.01.35) | Confirmed (NeoRead) |
| `0x06D` | 4 | Hardware version | Confirmed (NeoRead) |
| `0x071..0x13F` | | Other header values; includes an ascending u16 table (ramp?) | Guess |
| `0x140` | 128 | Settings block (`C5 C5`, see below) | Confirmed |
| `0x1C0..0x7FF` | | `0xFF` padding | Confirmed |
| `0x800` | 812 × n | One record per second of therapy | Confirmed |

`(fileSize - 0x800) % 812 == 0` for every file. A parser should read byte 8
and refuse (or handle) version 1 files instead of assuming 812-byte records.

#### 812-byte record

| Offset | Size | Meaning | Confidence |
| --- | --- | --- | --- |
| 0 | 2 | Magic `4D 4D` (`MM`) | Confirmed |
| 2 | 2 | `00 00` | Confirmed (always zero so far) |
| 4 | 7 | Timestamp: year u16, month, day, hour, minute, second | Confirmed |
| 11 | 1 | Weekday, Monday = 0 | Confirmed |
| 12 | 800 | 400 × u16 samples, laid out below (slot `s` is at byte 12 + 2s) | |

Timestamps skip one second about every 341 s (clock drift); this is not
missing data. **Usage time = sum of active intervals**: split the record
stream where timestamps jump by more than a minute (mask off) and add up the
intervals. Last-minus-first overstates usage on nights with pauses (one
night spans 678 min but the machine reports 466).

NeoRead fills gaps between records with repeats of the last value for its
graphs. It ignores the record's year and month and uses only day, hour,
minute and second. SleepLens should not copy the gap filling: a gap is mask
off, and should show as a gap.

#### Sample slots (index into the 400 u16 values)

Slots 20-369 are 50 Hz waveforms (50 samples = 1 s), continuous across
records. Breathing shows up at ~0.3 Hz in every breathing channel.

| Slots | Rate | Meaning | Scale | Confidence |
| --- | --- | --- | --- | --- |
| 0-9, 10-19 | 10 Hz × 2 | Raw sensor counts (two sensors) | raw ADC | Likely |
| 20-69 | 50 Hz | **Set (target) pressure** | ÷10 → cmH2O | Confirmed (matches summary mean and min/max settings). NeoRead does not plot it |
| 70-119 | 50 Hz | **Measured (feedback) pressure**. NeoRead's pressure graph uses this channel, not the set pressure | ÷10 → cmH2O | Confirmed (NeoRead) |
| 120-169 | 50 Hz | Measured pressure, mask side | ÷10 → cmH2O | Likely. NeoRead does not plot it |
| 170-219 | 50 Hz | **Snore** (breath-synced, spiky; correlates 0.66 with signed flow) | raw, 0-200 | Confirmed (NeoRead) |
| 220-269 | 50 Hz | **Patient flow** | (v − 500) → L/min | Confirmed (NeoRead); also gives 0.38 L tidal volume, 19 breaths/min |
| 270-319 | 50 Hz | **Leak flow**, slowly updated, 0-87 on the sample card; `E8` ≈ floor(1-min mean ÷ 5) | L/min, as stored | Confirmed (NeoRead) |
| 320-369 | 50 Hz | Total flow (patient + leak). NeoRead skips it | (v − 500) → L/min | Likely |
| 370-399 | 1 Hz | Per-second status values, below. NeoRead skips them | | |

**Two leak channels.** NeoRead calls slots 270-319 "leak flow". Our earlier
guess was slot 384 (correlates 0.90 with total minus patient flow; median
~29 L/min). They may be different things, for example unintentional leak
versus total leak including the mask vent. Which one matches the machine's
screen is still to be checked against the real card. Until then, use slots
270-319 as "leak" (the vendor's choice) and keep slot 384 as a second,
unlabelled channel.

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
| 384 | Leak-like value, L/min (correlates 0.90 with total minus patient flow). See "Two leak channels" above | Likely (leak), Unresolved (which leak) |
| 391 | 0-10000; tracks 375/373, not pressure: thermal control? | Guess |
| 370, 371, 373, 377 | Slowly varying raw values (sensor references?) | Guess |
| 372 | Signed i16, −2071…2146 | Guess |
| others | Always zero so far | |

### `.bkp` log

#### NeoRead's address map

NeoRead reads the `.bkp` at fixed addresses (Confirmed (NeoRead)):

| Address | Contents |
| --- | --- |
| `0x2000` | Device info block, same logo / device ID / type / version layout as the `.dat` header. If `0x2000` is `FF`, use `0x3000`; otherwise walk 256-byte entries and take the last one before `FF` |
| `0x20000-0x50000` | Index items, 512 bytes each: `A5 A5` item (64 bytes), then `B5 B5` statistics (64 bytes), then `C5 C5` therapy settings at +128 |
| `0x50000-0x350000` | Therapy events: 8192-byte day blocks of 8-byte entries |

#### Block types

Inside those areas the log is made of 128-byte blocks. The first bytes give
the block type:

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
text lines (11 such blocks on the sample card). Using NeoRead's address map
first avoids most of this guessing.

The log is a ring buffer: on the sample card, session summaries cover the
last ~12 months and event entries the last ~14 months. **Older nights have
waveforms in `.dat` but no machine-scored AHI.**

#### `A5 A5` session summary (index formulas checked on 379 nights)

Offsets from the start of the `A5 A5` block. The `B5 B5` statistics start at
`0x40`. NeoRead's field names are in brackets. Scales are for firmware
01.01.19 and newer (the sample card).

| Offset | Size | Meaning | Confidence |
| --- | --- | --- | --- |
| 0 | 2 | `A5 A5` | Confirmed |
| 2 | 2 | `01 01` | Confirmed |
| 4 | 2 | Session number u16 | Likely |
| 6 | 7 | Session start: year u16, month, day, hour, minute, second | Confirmed |
| 13 | 1 | Weekday | Confirmed |
| 0x40 | 2 | `B5 B5` | Confirmed |
| 0x42 | 7 | Same start timestamp (year u16, month at 0x44, day at 0x45, …), then `FF` | Confirmed |
| 0x4A | 2 | Item number u16 [ItemNo] | Confirmed (NeoRead) |
| 0x4C | 2 | **Start pressure** ×10 [InitPressure]. Earlier read as "minimum pressure" | Confirmed (NeoRead) |
| 0x4E | 2 | Mean pressure ×10, truncated [AvgeTreatPressure] (6.7 stored for a computed 6.77) | Confirmed |
| 0x50 | 2 | **P90 pressure** ×10 [P90]. Coarse 0.5 steps on the sample card (matches per-second P90 rounded to 0.5 on 366/379 nights) | Confirmed (NeoRead) |
| 0x52 | 2 | **Large-leak minutes** ×10: minutes with leak above 90 L/min [LeakMinutes]. Always 0 on the sample card so far | Confirmed (NeoRead) |
| 0x54 | 2 | **Usage minutes** [TreatMinutes] (504 for a 23:13:58-07:37:13 night) | Confirmed |
| 0x56 | 2 | **SNI (snore index)** ×10 [SNI]. Always 0 on the sample card so far | Confirmed (NeoRead) |
| 0x58 | 2 | **AHI ×10** = round-half-up((E1+E2+E3) × 600 / usage min) [AHI] | Confirmed, 379/379 |
| 0x5A | 2 | Apnea index ×10, E1+E2 [AI] | Confirmed, 379/379 |
| 0x5C | 2 | **Central apnea index** ×10, E2 [CAI] | Confirmed, 379/379; meaning Confirmed (NeoRead) |
| 0x5E | 2 | Highest pressure reached ×10, not the configured max [MaxP] | Confirmed (NeoRead); matches 358/379 |
| 0x60 | 2 | Hypopnea index ×10, E3 [HI] | Confirmed, 379/379 |

For `0x5A`-`0x60`, NeoRead treats `FF FF` as 0.

**Older firmware (below 01.01.19)** uses other scales: mean pressure and P90
in 0.5 cmH2O steps (value × 0.5), large-leak minutes as a whole number, SNI and AHI as whole
numbers, and no AI, CAI, MaxP or HI. Read the firmware version from the
header before choosing scales. The sample card is newer.

#### 8-byte therapy event entries

```
[yy] [ss] [MM] [dd] [code] [hh] [mi] [value]
```

`yy` is year − 2000 on this firmware. NeoRead uses codes `A0`-`AF`; the
sample card's newer firmware uses `E0`-`EF`. `E0`-`E3` line up with
`A0`-`A3` (and `E2` matches the central apnea index), so the rest most
likely follow the same order, shifted by `0x40`.

NeoRead's codes: `A0` pressure, `A1` obstructive apnea, `A2` central apnea,
`A3` hypopnea, `A4` snore level, `A5` big leak, `A6` flow limitation, `A7`
snore count, `AE` treatment start, `AF` treatment end.

| Code | Meaning | `value` | Confidence |
| --- | --- | --- | --- |
| `E0` | Pressure change | new pressure in 0.5 cmH2O steps (10 → 5.0) | Likely (NeoRead, older firmware) |
| `E1` | **Obstructive apnea** | pressure at event (0.5 steps) | Confirmed (NeoRead) |
| `E2` | **Central apnea** | pressure at event | Confirmed (NeoRead) |
| `E3` | Hypopnea | value (0.5 steps) | Confirmed |
| `E4` | Snore level | | Likely (NeoRead, older firmware) |
| `E5` | Big leak | | Likely (NeoRead, older firmware) |
| `E6` | Flow limitation | | Likely (NeoRead, older firmware) |
| `E7` | Snore count. Earlier read as a session marker | | Likely (NeoRead, older firmware) |
| `E8` | Level 0-14 derived from slots 270-319 (NeoRead's leak flow), so most likely a **leak level**. No NeoRead equivalent | level | Likely (derivation), Unresolved (meaning) |
| `EE` | Treatment start | | Likely (NeoRead, older firmware) |
| `EF` | Treatment end | | Likely (NeoRead, older firmware) |

Events are stamped at the **end** of the event: flow amplitude drops below
~15% of normal for 10-20 s and recovers right after the stamp. E1 + E2 per
night match the apnea index, E3 matches the hypopnea index, E2 alone matches
the central apnea index.

The sample data agrees with NeoRead's labels: after E1 the machine raises
target pressure in 87% of cases (median +1.9 cmH2O, n=304); after E2 in 24%
(median 0.0, n=911). Auto-CPAPs raise pressure for obstructive events and
hold it for central ones.

**Day assignment.** NeoRead assigns an event day block to the previous date
when the block's first entry is before 12:00 (hour < 12). This matches our
rule that a night belongs to the evening it started.

#### `C5 C5` settings snapshot

Offsets from the start of the `C5 C5` block. Field names are NeoRead's.

| Offset | Storage | Meaning | Confidence |
| --- | --- | --- | --- |
| `0x00` | `C5 C5` | Magic | Confirmed |
| `0x02` | u8 | **Mode**: 0 CPAP, 1 Auto CPAP, 2 AUTOS, 3 S, 4 ST | Confirmed (NeoRead) |
| `0x03` | i16 ×10 | **Start pressure** [StartPressure] | Confirmed (NeoRead) |
| `0x05` | i16 ×10 | Fixed CPAP pressure [TreatPressure] | Confirmed (NeoRead) |
| `0x07` | i16 ×10 | **Minimum pressure** [MinTreatPressure] | Confirmed (NeoRead) |
| `0x09` | i16 ×10 | **Maximum pressure** [MaxTreatPressure] | Confirmed (NeoRead) |
| `0x0B` | u8 | Breath rate (bilevel only) | Confirmed (NeoRead) |
| `0x0C..0x13` | u16 × 4 | Inhale time, max and min inhale time, trigger sensitivity (bilevel only) | Confirmed (NeoRead) |
| `0x14` | u8 | Comfort (AS-Elex) time | Confirmed (NeoRead) |
| `0x15` | low 4 bits | **Comfort (AS-Elex) level** (byte & `0x0F`); 3 on the sample card | Confirmed (NeoRead) |
| `0x16` | u8 | Leak alert, 0 = off | Confirmed (NeoRead) |
| `0x17` | u8, 1-5 | **Humidifier level** (13/14 text-log changes match) | Confirmed |
| `0x18` | u8 | **Ramp time**, minutes; 15 on the sample card | Confirmed (NeoRead) |
| `0x19` | u8 | Auto ON | Confirmed (NeoRead) |
| `0x1A` | u8 | Auto OFF | Confirmed (NeoRead) |
| `0x1B` | u8 | Language | Confirmed (NeoRead) |
| `0x1C` | u8 | Backlight | Confirmed (NeoRead) |
| `0x1D` | u8 | Pressure unit (hPa or cmH2O; enum values not yet known) | Confirmed (NeoRead) field, Unresolved values |
| `0x1E` | u8 | Time format | Confirmed (NeoRead) |
| `0x1F` | u16 | Settings version | Confirmed (NeoRead) |
| `0x21` | 7 | Timestamp (NeoRead skips `0x21..0x28`) | Confirmed |
| `0x28` | u8 | Weekday, Monday = 0 | Confirmed |
| `0x29..` | | Rise time, target tidal volume, min exhale, max and min inhale, inhale pressure (bilevel only) | Confirmed (NeoRead) |
| `0x2E` | u8, always 3 | Inside the bilevel fields; meaning on an auto CPAP unknown (not humidifier) | Unresolved |
| `0x7F` | `88` | Constant end byte | Confirmed |

On the sample card `0x03` and `0x07` are always equal: start pressure and
minimum pressure happen to be set to the same value. This replaces the old
guess that both meant "minimum and start pressure". Settings never changed
deliberately on the sample card, so the new fields have not yet been seen
changing.

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

### NeoRead report rules

- **Valid day** = usage over 4 hours. NeoRead counts these as "valid days".
- The report shows: mode, start pressure, min and max pressure, AS-Elex mode
  and level, leak alert, humidifier, ramp, Auto ON/OFF and pressure unit;
  then hours used, valid days, max pressure, mean pressure, P90, AHI, AI, HI
  and CAI.

### Open questions

Settled by NeoRead:

- ~~E1 = obstructive, E2 = central~~: Confirmed (NeoRead), via the `0x5C`
  central apnea index.
- ~~Remaining `C5 C5` fields (ramp, comfort level, Auto ON/OFF)~~: mapped
  from NeoRead's field list above.
- ~~Meaning of slots 170-219~~: snore.
- ~~Meaning of slots 270-319~~: leak flow, L/min.

Still need the real card (NeoRead runs on Windows only):

1. Does the card's newer firmware really use `E4`-`E7` like NeoRead's
   `A4`-`A7`? (`E7` was earlier read as a session marker.) What is `E8`;
   is it a leak level?
2. Which leak matches the machine's screen: slots 270-319 or slot 384? Is
   one total leak and the other unintentional leak?
3. Pressure unit (`C5 C5` `0x1D`): which value means hPa and which cmH2O?
4. Confirm SNI (`0x56`) and large-leak minutes (`0x52`) against NeoRead's
   report for the same night. Both read 0 on the sample card so far.

## ResMed AirSense 10 / 11

Public EDF+ format, documented by OSCAR. `STR.edf` has one row per day with
summary statistics (usage, AHI, leak, pressure percentiles); `DATALOG/<date>/`
has per-session EDF files (`BRP` flow/pressure 25 Hz, `PLD` 0.5 Hz, `EVE`
events, `SA2` oximetry). Parsing is mostly straightforward with an EDF reader.
