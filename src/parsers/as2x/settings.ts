import type { PressureUnitLabel, SettingsSnapshot, TherapyModeLabel } from './types';
import { makeDataView, readTimestamp } from './time';

export const BLOCK_SIZE = 128;

/** Settings mode byte (0x02) to NeoRead's label. */
export const THERAPY_MODES: readonly TherapyModeLabel[] = ['CPAP', 'Auto CPAP', 'AUTOS', 'S', 'ST'];

export function therapyModeLabel(mode: number): TherapyModeLabel {
  return THERAPY_MODES[mode] ?? 'unknown';
}

/**
 * Settings byte 0x1D to a unit label. 1 = hPa (Confirmed: the machine screen
 * shows hPa with this byte). The cmH2O byte value has not been seen on a card
 * yet, so every other value is 'unknown' rather than a guess.
 */
export function pressureUnitLabel(unit: number): PressureUnitLabel {
  return unit === 1 ? 'hPa' : 'unknown';
}

/**
 * Parse a 128-byte `C5 C5` settings block at `offset` with NeoRead's layout.
 * Returns null if the magic is wrong.
 */
export function parseSettingsBlock(buf: Uint8Array, offset: number): SettingsSnapshot | null {
  if (offset + BLOCK_SIZE > buf.length) return null;
  if (buf[offset] !== 0xc5 || buf[offset + 1] !== 0xc5) return null;
  const dv = makeDataView(buf);
  const pressureFields: number[] = [];
  for (const rel of [3, 5, 7, 9]) pressureFields.push(dv.getUint16(offset + rel, true));
  const p = (rel: number) => dv.getInt16(offset + rel, true) / 10;
  const u8 = (rel: number) => buf[offset + rel]!;
  const mode = u8(0x02);
  return {
    time: readTimestamp(dv, offset + 0x21),
    pressureFields,
    mode,
    modeLabel: therapyModeLabel(mode),
    startPressure: p(0x03),
    fixedPressure: p(0x05),
    minPressure: p(0x07),
    maxPressure: p(0x09),
    comfortTime: u8(0x14),
    comfortLevel: u8(0x15) & 0x0f,
    leakAlert: u8(0x16),
    humidifierLevel: u8(0x17),
    rampMinutes: u8(0x18),
    autoOn: u8(0x19) !== 0,
    autoOff: u8(0x1a) !== 0,
    pressureUnit: u8(0x1d),
    pressureUnitLabel: pressureUnitLabel(u8(0x1d)),
    raw: buf.slice(offset, offset + BLOCK_SIZE),
  };
}
