import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { buildSyntheticCard, indexX10 } from '../tools/lib/synthetic';
import {
  detectAs2x,
  eventTypeForCode,
  formatDeviceVersion,
  iterateDatRecords,
  modelNameForId,
  parseBkp,
  parseDatHeader,
  parseSettingsBlock,
  parseTherapyEntry,
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
    expect(h.formatVersion).toBe(2);
    expect(h.recordsSupported).toBe(true);
    expect(h.deviceModel).toBe('AS2X');
    expect(h.modelName).toBe('AS100A');
    expect(h.serial).toBe('TEST0000');
    expect(h.deviceType).toBe(1);
    expect(h.softwareVersion).toBe('9.01.00.00');
    expect(h.hardwareVersion).toBe('2.00');
    expect(h.recordCount).toBe(600);
    expect(h.trailingBytes).toBe(0);
    expect(h.created?.iso).toBe('2030-01-15T22:30:00');
    expect(h.settings?.pressureFields).toEqual([50, 60, 45, 80]);
    // Humidifier is byte 0x17 (4), not 0x2E (3).
    expect(h.settings?.humidifierLevel).toBe(4);
    expect(h.settings?.raw[0x2e]).toBe(3);
  });

  it('decodes settings with the NeoRead layout', () => {
    const s = parseDatHeader(N1).settings!;
    expect(s).toMatchObject({
      mode: 1,
      modeLabel: 'Auto CPAP',
      startPressure: 5, // 0x03
      fixedPressure: 6, // 0x05
      minPressure: 4.5, // 0x07 (the old parser had 0x03 as the minimum)
      maxPressure: 8, // 0x09
      comfortTime: 3,
      comfortLevel: 2, // 0x22 & 0x0F
      leakAlert: 1,
      humidifierLevel: 4,
      rampMinutes: 15,
      autoOn: true,
      autoOff: false,
      pressureUnit: 0,
    });
    expect(s.time?.iso).toBe('2030-01-15T22:30:00');
    const b = new Uint8Array(128);
    b[0] = b[1] = 0xc5;
    b[2] = 7;
    expect(parseSettingsBlock(b, 0)!.modeLabel).toBe('unknown');
    expect(parseSettingsBlock(b.fill(0, 0, 2), 0)).toBeNull();
  });

  it('maps device IDs to model names and formats versions like NeoRead', () => {
    expect(modelNameForId('AS1X')).toBe('AS100C');
    expect(modelNameForId('AS2X')).toBe('AS100A');
    expect(modelNameForId('AS3')).toBe('AS100A');
    expect(modelNameForId('AS4x')).toBe('AS100A-R');
    expect(modelNameForId('AS5')).toBe('AS100A-RT');
    expect(modelNameForId('AS6')).toBe('AS100C');
    expect(modelNameForId('AS7')).toBe('AS100C-R');
    expect(modelNameForId('AS8')).toBe('AS100C-RT');
    expect(modelNameForId('AS9X')).toBeNull();
    expect(modelNameForId('')).toBeNull();
    // Least significant byte first; parts after the first are two digits.
    expect(formatDeviceVersion([35, 1, 1, 0])).toBe('1.01.35');
    expect(formatDeviceVersion([4, 3, 2, 1])).toBe('1.02.03.04');
    expect(formatDeviceVersion([5, 1, 0, 0])).toBe('1.05');
    expect(formatDeviceVersion([0, 0, 0, 0])).toBeNull();
    expect(formatDeviceVersion([255, 255, 255, 255])).toBeNull();
  });

  it('reports a version 1 night file without decoding its records', () => {
    const v1 = new Uint8Array(0x800 + 512 * 3 + 10);
    v1.set(N1.subarray(0, 0x800));
    v1[8] = 1;
    v1[0x800] = v1[0x801] = 0x3c; // `<<`
    const h = parseDatHeader(v1);
    expect(h.formatVersion).toBe(1);
    expect(h.recordsSupported).toBe(false);
    expect(h.recordCount).toBe(3);
    expect(h.trailingBytes).toBe(10);
    expect([...iterateDatRecords(v1)]).toEqual([]);
    const s = summarizeDat(v1);
    expect(s.recordSeconds).toBe(0);
    expect(s.start).toBeNull();
  });

  it('streams records with decoded channels', () => {
    const it = iterateDatRecords(N1, { waveforms: true });
    const first = it.next().value!;
    expect(first.time?.iso).toBe('2030-01-15T22:30:00');
    expect(first.weekday).toBe(1); // Tuesday
    expect(first.setPressure).toBe(5);
    expect(first.leak).toBe(20);
    expect(first.leakFlow).toBe(10); // slots 270-319: leakBase - 10 + (i % leakMod), +-1 alternating
    expect(first.snore).toBeGreaterThan(0);
    expect(first.waveforms!.setPressure.length).toBe(50);
    expect(first.waveforms!.setPressure[0]).toBe(5);
    expect(first.waveforms!.leakFlow[0]).toBe(9);
    expect(first.waveforms!.leakFlow[1]).toBe(11);
    expect(first.waveforms!.snore.length).toBe(50);
    expect(first.waveforms!.pressureFeedback[0]).toBeGreaterThan(5);
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
    expect(amp[125]).toBeLessThan(10); // obstructive apnea ends at record 130
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
    // Leak flow 10-19 L/min, plus 90 s at 95 L/min (the large-leak window).
    expect(s.leakFlow).toEqual({ median: 15, p95: 95, max: 95 });
    expect(s.series!.leakFlow[450]).toBe(95);
    expect(s.series!.leakFlow[3]).toBe(13);
    expect(s.series!.snore.length).toBe(600);
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
    expect(s.leakFlow).toEqual({ median: 25, p95: 31, max: 31 });
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
    // 0x58 AHI, 0x5A apnea index (E1+E2), 0x5C central apnea index (E2), 0x60 hypopnea index, all x10.
    expect(a).toMatchObject({
      sessionNumber: 1,
      usageMinutes: 11,
      startPressure: 5,
      largeLeakMinutes: 1.5,
      snoreIndex: 1.2,
      ahi: 16.4,
      apneaIndex: 10.9,
      centralApneaIndex: 5.5,
      apnea2Index: 5.5, // deprecated alias
      // 10.9 - 5.5: one off the E1 count's own index (5.5) because both inputs are rounded.
      obstructiveApneaIndex: 5.4,
      hypopneaIndex: 5.5,
      observedMaxPressure: 8,
      hasB5Marker: true,
    });
    expect(a!.start!.iso).toBe('2030-01-15T22:30:00');
    expect(a!.pressure).toEqual({ min: 5, mean: 6.4, p90: 8 }); // min: deprecated alias of startPressure
    expect(b).toMatchObject({
      sessionNumber: 2,
      usageMinutes: 12,
      largeLeakMinutes: 0,
      snoreIndex: 0,
      ahi: 10,
      apneaIndex: 5,
      centralApneaIndex: 0,
      obstructiveApneaIndex: 5,
      hypopneaIndex: 5,
      observedMaxPressure: 7.5,
    });
    expect(bkp.sessions[2]).toMatchObject({
      sessionNumber: 3,
      usageMinutes: 11,
      ahi: 21.8,
      centralApneaIndex: 10.9,
      obstructiveApneaIndex: 5.5,
      snoreIndex: 3.5,
    });
  });

  it('reads 0xFFFF in the index fields as 0, as NeoRead does', () => {
    const card = new Uint8Array(BKP);
    const o = [...Array(card.length / 128).keys()].map((k) => k * 128).find((x) => card[x] === 0xa5 && card[x + 1] === 0xa5)!;
    for (const rel of [0x5a, 0x5c, 0x5e, 0x60]) card[o + rel] = card[o + rel + 1] = 0xff;
    const s = parseBkp(card).sessions[0]!;
    expect(s).toMatchObject({
      apneaIndex: 0,
      centralApneaIndex: 0,
      obstructiveApneaIndex: 0,
      observedMaxPressure: 0,
      hypopneaIndex: 0,
      ahi: 16.4,
    });
  });

  it('keeps index fields consistent with counts (round half up)', () => {
    expect(indexX10(3, 11)).toBe(164);
    expect(indexX10(1, 12)).toBe(50);
    expect(indexX10(1, 8)).toBe(75);
    expect(indexX10(1, 1200)).toBe(1); // 0.5 rounds up
  });

  it('decodes therapy events', () => {
    const count = (type: string) => bkp.events.filter((e) => e.eventType === type).length;
    expect(bkp.events).toHaveLength(36);
    expect(count('obstructiveApnea')).toBe(3);
    expect(count('centralApnea')).toBe(3);
    expect(count('hypopnea')).toBe(3);
    expect(count('pressureChange')).toBe(12);
    expect(count('level')).toBe(3);
    expect(count('treatmentStart')).toBe(3);
    expect(count('treatmentEnd')).toBe(3);
    expect(count('snoreLevel')).toBe(2);
    expect(count('snoreCount')).toBe(1);
    expect(count('largeLeak')).toBe(1);
    expect(count('flowLimitation')).toBe(2);
    expect(count('unknown')).toBe(0);
    // E8 range is 0-14.
    expect(bkp.events.filter((e) => e.eventType === 'level').map((e) => e.value).sort((x, y) => x - y)).toEqual([2, 9, 14]);
    const central = bkp.events.find((e) => e.eventType === 'centralApnea')!;
    expect(central.code).toBe(0xe2);
    expect(bkp.events.find((e) => e.eventType === 'obstructiveApnea')!.code).toBe(0xe1);
    expect(central.time.iso).toBe('2030-01-15T22:34:10');
    expect(central.pressure).toBe(6.5);
    const leak = bkp.events.find((e) => e.eventType === 'largeLeak')!;
    expect(leak).toMatchObject({ code: 0xe5, value: 1 });
    expect(leak.pressure).toBeUndefined();
    expect(bkp.events.find((e) => e.eventType === 'snoreLevel')!.value).toBe(3);
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
    const start = bkp.events.find((e) => e.time.iso === '2030-01-17T21:15:08' && e.eventType === 'treatmentStart');
    expect(start).toBeDefined();
    expect(bkp.events.filter((e) => e.time.iso.startsWith('2030-01-17'))).toHaveLength(11);
  });

  it('maps event codes: NeoRead 0xA0-0xAF and newer 0xE0-0xEF, nothing else', () => {
    const pairs: Array<[number, string]> = [
      [0x0, 'pressureChange'],
      [0x1, 'obstructiveApnea'],
      [0x2, 'centralApnea'],
      [0x3, 'hypopnea'],
      [0x4, 'snoreLevel'],
      [0x5, 'largeLeak'],
      [0x6, 'flowLimitation'],
      [0x7, 'snoreCount'],
      [0xe, 'treatmentStart'],
      [0xf, 'treatmentEnd'],
    ];
    for (const [nib, type] of pairs) {
      expect(eventTypeForCode(0xa0 | nib)).toBe(type);
      expect(eventTypeForCode(0xe0 | nib)).toBe(type);
    }
    expect(eventTypeForCode(0xe8)).toBe('level');
    expect(eventTypeForCode(0xa8)).toBe('unknown');
    expect(eventTypeForCode(0xe9)).toBe('unknown');
    for (const code of [0x00, 0x9f, 0xb0, 0xc5, 0xdf, 0xf0, 0xff]) expect(eventTypeForCode(code)).toBeNull();
  });

  it('parses A-code entries in both the yy and the older u16-year layout', () => {
    // [yy][ss][MM][dd][code][hh][mi][value]
    const newer = Uint8Array.of(30, 15, 2, 3, 0xa2, 1, 2, 12);
    expect(parseTherapyEntry(newer, 0)).toMatchObject({
      eventType: 'centralApnea',
      code: 0xa2,
      value: 12,
      pressure: 6,
      time: { iso: '2030-02-03T01:02:15' },
    });
    // Older firmware: u16 year 2030 (0x07EE), no seconds.
    const older = Uint8Array.of(0xee, 0x07, 2, 3, 0xa1, 1, 2, 12);
    expect(parseTherapyEntry(older, 0)).toMatchObject({ eventType: 'obstructiveApnea', time: { iso: '2030-02-03T01:02:00' } });
    // Codes outside A0-AF / E0-EF are rejected, as is an empty entry.
    expect(parseTherapyEntry(Uint8Array.of(30, 15, 2, 3, 0xf1, 1, 2, 12), 0)).toBeNull();
    expect(parseTherapyEntry(Uint8Array.of(30, 15, 2, 3, 0x91, 1, 2, 12), 0)).toBeNull();
    expect(parseTherapyEntry(Uint8Array.of(0xff, 15, 2, 3, 0xa1, 1, 2, 12), 0)).toBeNull();
  });

  it('reads a block of A-code entries as events without disturbing other blocks', () => {
    const block = new Uint8Array(128).fill(0xff);
    block.set(Uint8Array.of(30, 0, 3, 1, 0xae, 22, 0, 0), 0);
    block.set(Uint8Array.of(30, 0, 3, 1, 0xa4, 23, 0, 4), 8);
    block.set(Uint8Array.of(30, 0, 3, 2, 0xaf, 6, 0, 0), 16);
    const withA = new Uint8Array(BKP.length + 128);
    withA.set(BKP);
    withA.set(block, BKP.length);
    const r = parseBkp(withA);
    expect(r.events).toHaveLength(bkp.events.length + 3);
    expect(r.events.slice(-3).map((e) => e.eventType)).toEqual(['treatmentStart', 'snoreLevel', 'treatmentEnd']);
    expect(r.sessions).toHaveLength(3);
    expect(r.settings).toHaveLength(3);
    expect(r.counters).toHaveLength(3);
    expect(r.textEvents).toHaveLength(5);
    expect(r.stats.unknownBlocks).toBe(0);
    // One malformed entry (a code outside the ranges) makes the whole block unknown.
    block.set(Uint8Array.of(30, 0, 3, 2, 0xb1, 6, 0, 0), 24);
    withA.set(block, BKP.length);
    expect(parseBkp(withA).stats.unknownBlocks).toBe(1);
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
      expect(session.ahi * 10).toBe(indexX10(n('obstructiveApnea', 'centralApnea', 'hypopnea'), m));
      expect(session.apneaIndex * 10).toBe(indexX10(n('obstructiveApnea', 'centralApnea'), m));
      expect(Math.round(session.centralApneaIndex * 10)).toBe(indexX10(n('centralApnea'), m));
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
