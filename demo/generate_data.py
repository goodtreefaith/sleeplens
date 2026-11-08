#!/usr/bin/env python3
"""Generate made-up CPAP data for the SleepLens demo.

Everything here comes from a simple random model with a fixed seed. Nothing
is read from a real SD card. The output uses the same shapes the app's card
reader produces (a card summary with one entry per night, plus one detailed
night), so the demo page exercises the real UI code paths.

Usage: build.py calls generate(). Run this file directly to write
demo-data.json for inspection (not needed for the build, not committed).
"""
import datetime as dt
import json
import math
import os
import random

SEED = 20290101
START = dt.date(2029, 1, 1)
END = dt.date(2030, 3, 31)
SET_MIN, SET_MAX = 5.0, 10.0
SUMMARY_NIGHTS = 250          # the machine's summary log only covers recent nights
HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "demo-data.json")

rng = random.Random(SEED)


def clamp(v, lo, hi):
    return max(lo, min(hi, v))


def iso(t):
    return t.strftime("%Y-%m-%dT%H:%M:%S")


def pick_dates():
    """Calendar days with a recording: a holiday gap, a sick week, random skips."""
    gaps = set()
    for start, days in ((dt.date(2029, 8, 10), 12), (dt.date(2029, 11, 3), 6)):
        for k in range(days):
            gaps.add(start + dt.timedelta(days=k))
    out, d = [], START
    while d < END:  # the last night starts on END - 1 and finishes on END
        if d not in gaps and (rng.random() > 0.06 or d >= END - dt.timedelta(days=10)):
            out.append(d)
        d += dt.timedelta(days=1)
    return out


def make_sessions(day, minutes, nap=False, split=False):
    """Mask-on intervals for a night. Returns (sessions, first start, last end)."""
    bed = dt.datetime.combine(day, dt.time(22, 30)) + dt.timedelta(minutes=rng.gauss(0, 35))
    bed = bed.replace(second=rng.randrange(60), microsecond=0)
    sessions = []
    if nap:
        n0 = dt.datetime.combine(day, dt.time(14, rng.randrange(0, 40), rng.randrange(60)))
        nm = rng.uniform(35, 75)
        sessions.append((n0, n0 + dt.timedelta(minutes=nm)))
    if split and minutes > 120:
        first = minutes * rng.uniform(0.3, 0.7)
        brk = rng.uniform(4, 35)
        a_end = bed + dt.timedelta(minutes=first)
        b_start = a_end + dt.timedelta(minutes=brk)
        sessions.append((bed, a_end))
        sessions.append((b_start, b_start + dt.timedelta(minutes=minutes - first)))
    else:
        sessions.append((bed, bed + dt.timedelta(minutes=minutes)))
    out = [{"start": iso(a), "end": iso(b), "minutes": round((b - a).total_seconds() / 60, 1)} for a, b in sessions]
    return out, sessions[0][0], sessions[-1][1]


def event_counts(ahi, hours):
    total = max(0, round(ahi * hours))
    a1 = a2 = h = 0
    for _ in range(total):
        r = rng.random()
        if r < 0.5:
            a1 += 1
        elif r < 0.72:
            a2 += 1
        else:
            h += 1
    return a1, a2, h


def summary_block(day, start, usage_min, a1, a2, h, pmin, pmean, p90, pmax, number):
    hours = usage_min / 60
    return {
        "sessionNumber": number,
        "start": {"iso": iso(start)},
        "usageMinutes": round(usage_min),
        "pressure": {"min": pmin, "mean": pmean, "p90": p90},
        "observedMaxPressure": pmax,
        "ahi": round((a1 + a2 + h) / hours, 1) if hours else 0,
        "apneaIndex": round((a1 + a2) / hours, 1) if hours else 0,
        "apnea2Index": round(a2 / hours, 1) if hours else 0,
        "hypopneaIndex": round(h / hours, 1) if hours else 0,
    }


def detailed_night(day):
    """8 hours at 30 s resolution, events, and a 10 Hz flow excerpt around one apnea."""
    start = dt.datetime.combine(day, dt.time(22, 41, 20))
    n = 8 * 120
    # event times (seconds from start); the excerpt event is the type 2 apnea at ~4.5 h
    ev = [(2400, "hypopnea"), (4310, "apnea1"), (6620, "apnea1"), (9070, "apnea2"),
          (11800, "apnea1"), (14100, "hypopnea"), (16240, "apnea2"), (18760, "apnea1"),
          (20500, "hypopnea"), (22930, "apnea1"), (25700, "apnea2"), (27350, "hypopnea")]
    ev = [(t + rng.randrange(-40, 40), k) for t, k in ev]
    flow_event = 16240 + 17  # fixed so the excerpt lines up
    ev = [(flow_event, k) if k == "apnea2" and abs(t - 16240) < 60 else (t, k) for t, k in ev]
    series, p = [], 5.6
    mask_shift = (rng.randrange(15000, 17000), 26 * 60)  # one period where the mask moved
    for i in range(n):
        s = i * 30
        target = 5.8 + 0.9 * math.sin(s / 28800 * math.pi) + 0.25 * rng.gauss(0, 1)
        # auto CPAP: raises pressure after obstructive events and shallow breathing, not after central pauses
        for t, k in ev:
            if k != "apnea2" and 0 <= s - t < 1500:
                target += 1.6 * (1 - (s - t) / 1500)
        p += clamp(target - p, -0.2, 0.3)
        p = clamp(p, SET_MIN, SET_MAX)
        leak = 26 + 1.8 * rng.gauss(0, 1) + 2 * math.sin(s / 5400)
        if mask_shift[0] <= s < mask_shift[0] + mask_shift[1]:
            leak += 22 + 6 * rng.gauss(0, 1)
        amp = 22 + 2.5 * rng.gauss(0, 1)
        if any(0 <= t - s < 30 for t, _ in ev):
            amp = rng.uniform(3, 8)
        series.append({"t": (start + dt.timedelta(seconds=s)).strftime("%H:%M:%S"),
                       "p": round(p, 2), "leak": round(clamp(leak, 18, 70), 1), "flowAmp": round(clamp(amp, 2, 40), 1)})
    events = [{"t": (start + dt.timedelta(seconds=t)).strftime("%H:%M:%S"), "type": k} for t, k in sorted(ev)]

    # flow excerpt: 120 s at 10 Hz, event stamped at 90 s (its END), ~15 s of near-zero flow before it
    hz, data, t = 10, [], 0.0
    pause_a, pause_b = 74.5, 89.5
    phase = 0.0
    for i in range(120 * hz):
        t = i / hz
        if pause_a <= t < pause_b:
            data.append(round(rng.uniform(-1.2, 1.2)))
            continue
        period = 4.2 if t < pause_a else 3.6
        amp = 25 if t < pause_a - 12 else 25 - (t - (pause_a - 12)) * 1.0   # breaths fade before the pause
        if t >= pause_b:
            amp = 40 if t < pause_b + 4 else 27                              # deep recovery breath
            if t < pause_b + 0.1:
                phase = 0.0
        phase += 2 * math.pi / (period * hz)
        v = amp * math.sin(phase)
        v = v if v > 0 else v * 0.85
        data.append(round(v + rng.gauss(0, 0.8)))
    excerpt_start = start + dt.timedelta(seconds=flow_event - 90)
    flow = {"start": excerpt_start.strftime("%H:%M:%S"), "hz": hz, "eventAt": 90, "data": data}
    return start, series, events, flow


def generate():
    rng.seed(SEED)
    dates = pick_dates()
    detail_day = dates[-1]
    short = set(rng.sample(dates[:-1], 18))
    tests = set(rng.sample([d for d in dates[:-1] if d not in short], 1))
    naps = set(rng.sample([d for d in dates[:-1] if d not in short | tests], 2))
    nights = []
    first_summary = len(dates) - SUMMARY_NIGHTS
    for idx, day in enumerate(dates):
        has_summary = idx >= first_summary
        drift = idx / len(dates)
        if day == detail_day:
            start, series, events, flow = detailed_night(day)
            usage = 480.0
            ps = [s["p"] for s in series]
            leaks = sorted(s["leak"] for s in series)
            pmean = round(sum(ps) / len(ps), 1)
            p90 = round(sorted(ps)[int(len(ps) * 0.9)], 1)
            leak_med = round(leaks[len(leaks) // 2])
            a1 = sum(e["type"] == "apnea1" for e in events)
            a2 = sum(e["type"] == "apnea2" for e in events)
            h = sum(e["type"] == "hypopnea" for e in events)
            end = start + dt.timedelta(minutes=usage)
            sessions = [{"start": iso(start), "end": iso(end), "minutes": usage}]
            pmax = round(max(ps) * 2) / 2
        else:
            if day in tests:
                usage = rng.uniform(2, 6)
            elif day in short:
                usage = rng.uniform(80, 235)
            else:
                usage = clamp(rng.gauss(445, 55), 250, 590)
            sessions, start, end = make_sessions(day, usage, nap=day in naps, split=rng.random() < 0.28)
            usage = round(sum(s["minutes"] for s in sessions), 1)
            pmean = round(clamp(6.6 + 0.6 * drift + rng.gauss(0, 0.35), SET_MIN, SET_MAX - 1), 1)
            p90 = round(clamp(pmean + rng.uniform(1.0, 1.9), pmean, SET_MAX), 1)
            pmax = round(clamp(p90 + rng.uniform(0.2, 1.2), p90, SET_MAX) * 2) / 2
            leak_med = round(clamp(26 + rng.gauss(0, 2.4) + (rng.uniform(6, 9) if rng.random() < 0.03 else 0), 22, 35))
            r = rng.random()
            ahi = rng.uniform(6, 9) if r < 0.03 else clamp(rng.lognormvariate(math.log(1.6), 0.45), 0.5, 4)
            a1, a2, h = event_counts(ahi, usage / 60)
        night = {
            "date": day.strftime("%Y%m%d"),
            "start": iso(start), "end": iso(end),
            "usageMinutes": round(usage, 1),
            "pressureMean": pmean, "pressureP90": p90,
            "activeIntervals": len(sessions),
            "leakMedian": leak_med,
            "session": None, "eventCounts": None,
            "sessions": sessions,
        }
        if has_summary:
            night["session"] = summary_block(day, start, usage, a1, a2, h, SET_MIN, pmean, p90, pmax, idx + 1)
            night["eventCounts"] = {"apnea1": a1, "apnea2": a2, "hypopnea": h}
        nights.append(night)
    data = {
        "serial": "DEMO",
        "setting": {"min": SET_MIN, "max": SET_MAX},
        "nights": nights,
        "detail": {"date": detail_day.isoformat(), "series": series, "events": events, "flowExcerpt": flow},
    }
    return data


def main():
    data = generate()
    with open(OUT, "w") as f:
        json.dump(data, f, separators=(",", ":"))
    nights = data["nights"]
    print(f"wrote {OUT}: {len(nights)} nights, {sum(1 for n in nights if n['session'])} with machine summary, "
          f"detail night {data['detail']['date']} with {len(data['detail']['events'])} events")


if __name__ == "__main__":
    main()
