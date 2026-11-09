#!/usr/bin/env python3
"""Build the public SleepLens demo page.

Generates made-up data with generate_data.py (fixed seed, nothing from a real
card), inlines it into template.html and writes a single self-contained
index.html. Stdlib only.

Usage: python3 build.py
"""
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.dont_write_bytecode = True  # keep the repo free of __pycache__
sys.path.insert(0, HERE)
import generate_data  # noqa: E402

TEMPLATE = os.path.join(HERE, "template.html")
OUT = os.path.join(HERE, "index.html")
PLACEHOLDER = "/*__SLEEPLENS_DATA__*/null"


def iso_date(yyyymmdd):
    return f"{yyyymmdd[0:4]}-{yyyymmdd[4:6]}-{yyyymmdd[6:8]}"


def slim_night(n):
    s = n.get("session") or None
    ec = n.get("eventCounts") or None
    out = {
        "d": iso_date(n["date"]),
        "start": n["start"],
        "end": n["end"],
        "u": n["usageMinutes"],
        "pm": n["pressureMean"],
        "p90": n["pressureP90"],
        "lk": n["leakMedian"],
        "ahi": None,
        "ss": [[x["start"], x["end"], x["minutes"]] for x in (n.get("sessions") or []) if x.get("minutes", 0) >= 1],
    }
    if s:
        out["ahi"] = s.get("ahi")
        out["ai"] = s.get("apneaIndex")
        out["hi"] = s.get("hypopneaIndex")
        p = s.get("pressure") or {}
        out["ps"] = {"min": p.get("min"), "mean": p.get("mean"), "p90": p.get("p90")}
        out["mx"] = s.get("observedMaxPressure")
    if ec:
        out["a1"] = ec.get("obstructiveApnea", ec.get("apnea1", 0))
        out["a2"] = ec.get("centralApnea", ec.get("apnea2", 0))
        out["h"] = ec.get("hypopnea", 0)
    return out


def main():
    raw = generate_data.generate()
    nights = sorted((slim_night(n) for n in raw["nights"]), key=lambda n: n["d"])
    data = {
        "meta": {"setMin": raw["setting"]["min"], "setMax": raw["setting"]["max"], "demo": True},
        "nights": nights,
        "detail": raw["detail"],
    }
    payload = json.dumps(data, separators=(",", ":"), ensure_ascii=True).replace("</", "<\\/")
    with open(TEMPLATE) as f:
        html = f.read()
    if html.count(PLACEHOLDER) != 1:
        sys.exit("template placeholder missing or duplicated")
    with open(OUT, "w") as f:
        f.write(html.replace(PLACEHOLDER, payload))
    print(f"wrote {OUT} ({os.path.getsize(OUT) // 1024} KB, {len(nights)} nights)")


if __name__ == "__main__":
    main()
