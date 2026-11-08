import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { buildSyntheticCard } from '../tools/lib/synthetic';
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
    expect(h.settings?.humidifierRaw).toBe(3);
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
    expect(s.start?.iso).toBe('2030-01-15T22:30:00');
    expect(s.end?.iso).toBe('2030-01-15T22:40:01');
    expect(s.pressure).toEqual({ min: 5, mean: 6.4, p95: 8, max: 8 });
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
    expect(s.pressure).toEqual({ min: 5, mean: 6, p95: 7.5, max: 7.5 });
    expect(s.leak).toEqual({ median: 35, p95: 41, max: 41 });
  });
});

describe('bkp parsing', () => {
  const bkp = parseBkp(BKP);

  it('finds session summaries', () => {
    expect(bkp.sessions).toHaveLength(2);
    const [a, b] = bkp.sessions;
    expect(a).toMatchObject({
      sessionNumber: 1,
      usageMinutes: 11,
      ahi: 18,
      apneaIndex: 12,
      hypopneaIndex: 6,
      maxPressureSetting: 8,
      hasB5Marker: true,
    });
    expect(a!.start!.iso).toBe('2030-01-15T22:30:00');
    expect(a!.pressure).toEqual({ min: 5, mean: 6.4, p95: 8 });
    expect(b).toMatchObject({ sessionNumber: 2, usageMinutes: 12, ahi: 10, apneaIndex: 5, hypopneaIndex: 5 });
  });

  it('decodes therapy events', () => {
    const count = (type: string) => bkp.events.filter((e) => e.eventType === type).length;
    expect(bkp.events).toHaveLength(20);
    expect(count('apnea1')).toBe(2);
    expect(count('apnea2')).toBe(1);
    expect(count('hypopnea')).toBe(2);
    expect(count('pressureChange')).toBe(9);
    expect(count('level')).toBe(2);
    expect(count('sessionMarker')).toBe(4);
    const apnea2 = bkp.events.find((e) => e.eventType === 'apnea2')!;
    expect(apnea2.code).toBe(0xe2);
    expect(apnea2.time.iso).toBe('2030-01-15T22:34:10');
    expect(apnea2.pressure).toBe(6.5);
    const times = bkp.events.map((e) => e.time.epochSec);
    expect([...times].sort((x, y) => x - y)).toEqual(times);
  });

  it('decodes settings and text events', () => {
    expect(bkp.settings).toHaveLength(2);
    expect(bkp.settings[0]!.time!.iso).toBe('2030-01-15T22:30:00');
    expect(bkp.textEvents.map((t) => t.text)).toEqual([
      'SD card Insert',
      'Parameter change Humidifire 15 to 2',
      'Stop work by Auto Off in WorkScreen',
      'Machine Reset = 6  Software Version TEST-0.00',
    ]);
    expect(bkp.stats.emptyBlocks).toBe(4);
    expect(bkp.stats.unknownBlocks).toBe(0);
  });
});

describe('generator and parser agree', () => {
  const card = buildSyntheticCard();

  it('committed fixture equals freshly generated bytes', () => {
    expect(Buffer.compare(Buffer.from(card.get('TEST0000.bkp')!), BKP)).toBe(0);
    expect(Buffer.compare(Buffer.from(card.get('TEST0000203001/20300115.dat')!), N1)).toBe(0);
    expect(Buffer.compare(Buffer.from(card.get('TEST0000203001/20300116.dat')!), N2)).toBe(0);
  });

  it('summary AHI matches the logged events for each night', () => {
    const bkp = parseBkp(card.get('TEST0000.bkp')!);
    for (const path of ['TEST0000203001/20300115.dat', 'TEST0000203001/20300116.dat']) {
      const sum = summarizeDat(card.get(path)!, { includeSeries: false });
      const session = bkp.sessions.find((s) => Math.abs(s.start!.epochSec - sum.start!.epochSec) < 60)!;
      const n = bkp.events.filter(
        (e) =>
          ['apnea1', 'apnea2', 'hypopnea'].includes(e.eventType) &&
          e.time.epochSec >= sum.start!.epochSec &&
          e.time.epochSec <= sum.end!.epochSec,
      ).length;
      expect(session.ahi).toBeCloseTo(n / (sum.recordSeconds / 3600), 5);
      expect(session.usageMinutes).toBe(Math.ceil(sum.usageMinutes));
      expect(session.pressure.mean).toBeCloseTo(sum.pressure.mean, 5);
    }
  });

  it('detects the generated card', () => {
    expect(detectAs2x([...card.keys()]).nights).toHaveLength(2);
  });
});
