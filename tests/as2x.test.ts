import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { buildSyntheticCard, indexX10 } from '../tools/lib/synthetic';
import {
  detectAs2x,
  iterateDatRecords,
  parseBkp,
  parseDatHeader,
  summarizeDat,
} from '../src/parsers/as2x';

const dir = join(__dirname, '..', 'fixtures', 'synthetic-as2x');
const N1 = readFileSync(join(dir, 'TEST0000203001', '20300115.dat'));
const N2 = readFileSync(join(dir, 'TEST0000203001', '20300116.dat'));
const N3 = readFileSync(join(dir, 'TEST0000203001', '20300117.dat'));
const BKP = readFileSync(join(dir, 'TEST0000.bkp'));

describe('detectAs2x', () => {
  it('matches the card layout by names only', () => {
    const d = detectAs2x([
      'TEST0000.bkp',
      'TEST0000203001/20300116.dat',
      'TEST0000203001/20300115.dat',
      'notes.txt',
    ]);
    expect(d.detected).toBe(true);
    expect(d.serial).toBe('TEST0000');
    expect(d.bkp).toBe('TEST0000.bkp');
    expect(d.months).toEqual(['203001']);
    expect(d.nights.map((n) => n.date)).toEqual(['20300115', '20300116']);
  });
  it('rejects other layouts', () => {
    expect(detectAs2x(['DATALOG/20300101/x.edf', 'STR.edf']).detected).toBe(false);
    expect(detectAs2x(['ABC202501/20240101.dat']).detected).toBe(false);
  });
});

describe('dat parsing', () => {
  it('reads the header', () => {
    const h = parseDatHeader(N1);
    expect(h.deviceModel).toBe('AS2X');
    expect(h.serial).toBe('TEST0000');
    expect(h.recordCount).toBe(600);
    expect(h.trailingBytes).toBe(0);
    expect(h.created?.iso).toBe('2030-01-15T22:30:00');
    expect(h.settings?.pressureFields).toEqual([50, 60, 50, 80]);
    // Humidifier is byte 0x17 (4), not 0x2E (3).
    expect(h.settings?.humidifierLevel).toBe(4);
    expect(h.settings?.raw[0x2e]).toBe(3);
    expect(h.settings).toMatchObject({ minPressure: 5, startPressure: 5, maxPressure: 8 });
  });

  it('streams records with decoded channels', () => {
    const it = iterateDatRecords(N1, { waveforms: true });
    const first = it.next().value!;
    expect(first.time?.iso).toBe('2030-01-15T22:30:00');
    expect(first.weekday).toBe(1); // Tuesday
    expect(first.setPressure).toBe(5);
    expect(first.leak).toBe(20);
    expect(first.waveforms!.setPressure.length).toBe(50);
    expect(first.waveforms!.setPressure[0]).toBe(5);
    expect(Math.max(...first.waveforms!.patientFlow)).toBeGreaterThan(15);
    expect(first.flowAmplitude).toBeGreaterThan(20);
    // Clock-drift skip: record 341 is two seconds after record 340.
    let prev = first;
    for (const r of it) {
      if (r.index === 341) expect(r.time!.epochSec - prev.time!.epochSec).toBe(2);
      prev = r;
    }
  });

  it('flow is flat inside the apnea window', () => {
    const amp: number[] = [];
    for (const r of iterateDatRecords(N1)) amp.push(r.flowAmplitude);
    expect(amp[125]).toBeLessThan(10); // apnea1 ends at record 130
    expect(amp[100]).toBeGreaterThan(15);
  });

  it('summarizes night 1', () => {
    const s = summarizeDat(N1);
    expect(s.serial).toBe('TEST0000');
    expect(s.recordCount).toBe(600);
    expect(s.invalidRecords).toBe(0);
    expect(s.recordSeconds).toBe(600);
    expect(s.usageSeconds).toBe(601); // one clock-drift skip
    expect(s.usageMinutes).toBeCloseTo(601 / 60, 10);
    expect(s.sessions).toHaveLength(1); // the 1 s drift skip does not split the interval
    expect(s.sessions[0]!.minutes).toBeCloseTo(601 / 60, 10);
    expect(s.start?.iso).toBe('2030-01-15T22:30:00');
    expect(s.end?.iso).toBe('2030-01-15T22:40:01');
    expect(s.pressure).toEqual({ min: 5, mean: 6.4, p90: 8, p95: 8, max: 8 });
    expect(s.leak).toEqual({ median: 24, p95: 29, max: 29 });
    expect(s.series!.pressure.length).toBe(600);
    expect(s.series!.pressure[0]).toBe(5);
    expect(s.series!.pressure[599]).toBe(8);
    expect(s.series!.leak[3]).toBe(23);
  });

  it('summarizes night 2 without a series', () => {
    const s = summarizeDat(N2, { includeSeries: false });
    expect(s.series).toBeUndefined();
    expect(s.usageMinutes).toBe(12);
    expect(s.end?.iso).toBe('2030-01-16T23:17:00');
    expect(s.pressure).toEqual({ min: 5, mean: 6, p90: 7.5, p95: 7.5, max: 7.5 });
    expect(s.leak).toEqual({ median: 35, p95: 41, max: 41 });
  });
});

describe('night with a mid-night pause', () => {
  it('sums active intervals instead of last minus first', () => {
    const s = summarizeDat(N3, { includeSeries: false });
    expect(s.recordCount).toBe(600);
    expect(s.start?.iso).toBe('2030-01-17T21:15:08');
    expect(s.end?.iso).toBe('2030-01-17T21:55:09');
    // Wall clock is 2401 s (40 min); active time is 601 s.
    expect(s.end!.epochSec - s.start!.epochSec).toBe(2401);
    expect(s.usageSeconds).toBe(601);
    expect(s.usageMinutes).toBeCloseTo(601 / 60, 10);
    expect(s.sessions.map((x) => [x.start.iso, x.end.iso])).toEqual([
      ['2030-01-17T21:15:08', '2030-01-17T21:20:09'], // includes the drift skip at record 100
      ['2030-01-17T21:50:09', '2030-01-17T21:55:09'],
    ]);
    expect(s.sessions[0]!.minutes).toBeCloseTo(301 / 60, 10);
    expect(s.sessions[1]!.minutes).toBe(5);
    expect(s.pressure.p90).toBe(7);
  });
});

describe('bkp parsing', () => {
  const bkp = parseBkp(BKP);

  it('finds session summaries', () => {
    expect(bkp.sessions).toHaveLength(3);
    const [a, b] = bkp.sessions;
    // 0x58 AHI, 0x5A apnea index (E1+E2), 0x5C E2 index, 0x60 hypopnea index, all x10.
    expect(a).toMatchObject({
      sessionNumber: 1,
      usageMinutes: 11,
      ahi: 16.4,
      apneaIndex: 10.9,
      apnea2Index: 5.5,
      hypopneaIndex: 5.5,
      observedMaxPressure: 8,
      hasB5Marker: true,
    });
    expect(a!.start!.iso).toBe('2030-01-15T22:30:00');
    expect(a!.pressure).toEqual({ min: 5, mean: 6.4, p90: 8 });
    expect(b).toMatchObject({
      sessionNumber: 2,
      usageMinutes: 12,
      ahi: 10,
      apneaIndex: 5,
      apnea2Index: 0,
      hypopneaIndex: 5,
      observedMaxPressure: 7.5,
    });
    expect(bkp.sessions[2]).toMatchObject({ sessionNumber: 3, usageMinutes: 11, ahi: 21.8, apnea2Index: 10.9 });
  });

  it('keeps index fields consistent with counts (round half up)', () => {
    expect(indexX10(3, 11)).toBe(164);
    expect(indexX10(1, 12)).toBe(50);
    expect(indexX10(1, 8)).toBe(75);
    expect(indexX10(1, 1200)).toBe(1); // 0.5 rounds up
  });

  it('decodes therapy events', () => {
    const count = (type: string) => bkp.events.filter((e) => e.eventType === type).length;
    expect(bkp.events).toHaveLength(30);
    expect(count('apnea1')).toBe(3);
    expect(count('apnea2')).toBe(3);
    expect(count('hypopnea')).toBe(3);
    expect(count('pressureChange')).toBe(12);
    expect(count('level')).toBe(3);
    expect(count('sessionMarker')).toBe(6);
    // E8 range is 0-14.
    expect(bkp.events.filter((e) => e.eventType === 'level').map((e) => e.value).sort((x, y) => x - y)).toEqual([2, 9, 14]);
    const apnea2 = bkp.events.find((e) => e.eventType === 'apnea2')!;
    expect(apnea2.code).toBe(0xe2);
    expect(apnea2.likelyKind).toBe('central'); // provisional
    expect(bkp.events.find((e) => e.eventType === 'apnea1')!.likelyKind).toBe('obstructive'); // provisional
    expect(bkp.events.find((e) => e.eventType === 'hypopnea')!.likelyKind).toBeUndefined();
    expect(apnea2.time.iso).toBe('2030-01-15T22:34:10');
    expect(apnea2.pressure).toBe(6.5);
    const times = bkp.events.map((e) => e.time.epochSec);
    expect([...times].sort((x, y) => x - y)).toEqual(times);
  });

  it('decodes settings and text events', () => {
    expect(bkp.settings).toHaveLength(3);
    expect(bkp.settings[0]!.time!.iso).toBe('2030-01-15T22:30:00');
    expect(bkp.textEvents.map((t) => t.text)).toEqual([
      'SD card Insert',
      'Parameter change Humidifire 15 to 2',
      'Stop work by Auto Off in WorkScreen',
      'Machine Reset = 6  Software Version TEST-0.00',
      'Screen Filter to Reset',
    ]);
    expect(bkp.stats.emptyBlocks).toBe(4);
    expect(bkp.stats.unknownBlocks).toBe(0);
  });

  it('parses an event block whose first bytes read as a u16 year', () => {
    // Night 3 starts at second 8: its first entry is [30][08]... = year 2078 as a u16.
    let disguised = -1;
    for (let o = 0; o + 128 <= BKP.length; o += 128) {
      if (BKP[o] === 30 && BKP[o + 1] === 8 && BKP[o + 4]! >= 0xe0) disguised = o;
    }
    expect(disguised).toBeGreaterThanOrEqual(0);
    expect(BKP[disguised]! | (BKP[disguised + 1]! << 8)).toBe(2078);
    const start = bkp.events.find((e) => e.time.iso === '2030-01-17T21:15:08' && e.eventType === 'sessionMarker');
    expect(start).toBeDefined();
    expect(bkp.events.filter((e) => e.time.iso.startsWith('2030-01-17'))).toHaveLength(10);
  });

  it('parses counter blocks', () => {
    expect(bkp.stats.counterBlocks).toBe(2);
    expect(bkp.counters).toHaveLength(3); // the second block has one empty half
    expect(bkp.counters.map((c) => [c.date, c.counterA, c.counterB, c.counterC])).toEqual([
      ['2030-01-15', 1000, 5000, 4999],
      ['2030-01-16', 1200, 5200, 5199],
      ['2030-01-17', 0, 5400, 5399], // filter reset: A back to 0, B keeps counting
    ]);
    for (const c of bkp.counters) expect(c.counterC).toBe(c.counterB - 1);
  });
});

describe('generator and parser agree', () => {
  const card = buildSyntheticCard();

  it('committed fixture equals freshly generated bytes', () => {
    expect(Buffer.compare(Buffer.from(card.get('TEST0000.bkp')!), BKP)).toBe(0);
    expect(Buffer.compare(Buffer.from(card.get('TEST0000203001/20300115.dat')!), N1)).toBe(0);
    expect(Buffer.compare(Buffer.from(card.get('TEST0000203001/20300116.dat')!), N2)).toBe(0);
    expect(Buffer.compare(Buffer.from(card.get('TEST0000203001/20300117.dat')!), N3)).toBe(0);
  });

  it('summary indices match the logged events for each night', () => {
    const bkp = parseBkp(card.get('TEST0000.bkp')!);
    for (const path of ['TEST0000203001/20300115.dat', 'TEST0000203001/20300116.dat', 'TEST0000203001/20300117.dat']) {
      const sum = summarizeDat(card.get(path)!, { includeSeries: false });
      const session = bkp.sessions.find((s) => Math.abs(s.start!.epochSec - sum.start!.epochSec) < 60)!;
      const n = (...types: string[]) =>
        bkp.events.filter(
          (e) =>
            types.includes(e.eventType) &&
            e.time.epochSec >= sum.start!.epochSec &&
            e.time.epochSec <= sum.end!.epochSec,
        ).length;
      const m = session.usageMinutes;
      expect(session.ahi * 10).toBe(indexX10(n('apnea1', 'apnea2', 'hypopnea'), m));
      expect(session.apneaIndex * 10).toBe(indexX10(n('apnea1', 'apnea2'), m));
      expect(session.apnea2Index * 10).toBe(indexX10(n('apnea2'), m));
      expect(session.hypopneaIndex * 10).toBe(indexX10(n('hypopnea'), m));
      expect(session.usageMinutes).toBe(Math.ceil(sum.usageMinutes));
      expect(session.pressure.mean).toBeCloseTo(sum.pressure.mean, 1);
      expect(session.pressure.p90).toBe(Math.round(sum.pressure.p90 * 2) / 2);
    }
  });

  it('detects the generated card', () => {
    expect(detectAs2x([...card.keys()]).nights).toHaveLength(3);
  });
});
