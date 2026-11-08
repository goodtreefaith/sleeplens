# Card format notes

Working notes from inspecting real cards. Serial numbers are written as
`<SERIAL>`; never paste real serials, dates of birth or raw files here.

## AS2X-series auto CPAP (firmware `AM-1.01..47.1`)

The model string `AS2X` and firmware string are taken from the device's own
event log. We have not yet confirmed the brand or model sold to patients.
If you own one, please open an issue with the name on the label.

### Card layout

```
/<SERIAL><YYYYMM>/            one folder per month, e.g. ...202611/
    <YYYYMMDD>.dat            one file per therapy day, ~18-32 MB
/<SERIAL>.bkp                 4 MB event/settings log (ring buffer)
```

The 8-digit day in the file name is the evening the night *started*. A night
running 23:13 to 07:37 sits in the earlier day's file.

### `.dat` night file (first pass, unverified)

| Offset | Size | Meaning (hypothesis) |
| --- | --- | --- |
| `0x000` | 7 | Created timestamp: year u16 LE, month, day, hour, minute, second |
| `0x050` | 16 | ASCII `AS2X<SERIAL>` |
| `0x060..0x13F` | | Header: setting values; a run of ascending u16s looks like a pressure ramp table |
| `0x140` | 128 | Settings block, starts with magic `C5 C5` (same block type as in `.bkp`) |
| `0x1C0..0x7FF` | | `0xFF` padding |
| `0x800` | 812 × n | **One record per second** of therapy |

Each 812-byte record:

| Offset | Size | Meaning |
| --- | --- | --- |
| 0 | 2 | Magic `4D 4D` ("MM") |
| 2 | 2 | `00 00` (flags or channel set?) |
| 4 | 7 | Timestamp: year u16 LE, month, day, hour, minute, second |
| 11 | 1 | Weekday? (observed 5, 6) |
| 12 | 800 | 400 × u16 LE samples |

The 400 samples look like several channels laid out one after another,
probably 25 samples each (16 channels at 25 Hz) or a mix of rates. Seen in
one record: a run of `0x0032` (50, which could be 5.0 cmH2O pressure ×10),
a run of `0x004D`/`0x004E`, and a smoothly rising series (flow?). Pinning down
the channel map is Phase 0 of the plan.

A sample night: 30,104 records = 8 h 21 min, which matches the session time.

### `.bkp` log

- Repeated 128-byte `C5 C5` settings/event blocks with timestamps at offset
  `0x21` (year u16 LE at `0x21`, then month, day, hour, minute, second).
- Plain-text event lines, e.g. `Stop work by Auto Off in WorkScreen`,
  `Parameter change Humidifire 15 to 2`, `SD card Insert`,
  `Machine Reset = 6  Software Version AM-1.01..47.1`.
- These give mask-on/mask-off (Auto On/Off), humidifier changes and resets,
  useful for the usage timeline and a "settings changed" marker.

### Open questions

1. Channel map inside the 400 samples (pressure, flow, leak, tidal volume,
   respiratory rate, snore, flow limitation?).
2. Does the device store apnea/hypopnea events, or must they be derived from
   flow? No event strings like "apnea" appear in `.bkp`.
3. Units and scaling for each channel.
4. Meaning of `C5 C5` block fields (prescribed min/max pressure, ramp, EPR).

## ResMed AirSense 10 / 11

Public EDF+ format, documented by OSCAR. `STR.edf` has one row per day with
summary statistics (usage, AHI, leak, pressure percentiles); `DATALOG/<date>/`
has per-session EDF files (`BRP` flow/pressure 25 Hz, `PLD` 0.5 Hz, `EVE`
events, `SA2` oximetry). Parsing is mostly straightforward with an EDF reader.
