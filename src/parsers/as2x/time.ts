import type { LocalTime } from './types';

const pad = (n: number, w = 2) => String(n).padStart(w, '0');

/** Build a LocalTime, or null when the components are not a plausible date-time. */
export function makeLocalTime(
  year: number,
  month: number,
  day: number,
  hour: number,
  minute: number,
  second: number,
): LocalTime | null {
  if (year < 2000 || year > 2100) return null;
  if (month < 1 || month > 12 || day < 1 || day > 31) return null;
  if (hour > 23 || minute > 59 || second > 59) return null;
  const epochMs = Date.UTC(year, month - 1, day, hour, minute, second);
  // Rejects overflow like 31 February.
  if (new Date(epochMs).getUTCDate() !== day) return null;
  return {
    iso: `${pad(year, 4)}-${pad(month)}-${pad(day)}T${pad(hour)}:${pad(minute)}:${pad(second)}`,
    epochSec: epochMs / 1000,
    year,
    month,
    day,
    hour,
    minute,
    second,
  };
}

/** Read `year u16, month, day, hour, minute, second` (7 bytes) at `offset`. */
export function readTimestamp(dv: DataView, offset: number): LocalTime | null {
  if (offset + 7 > dv.byteLength) return null;
  return makeLocalTime(
    dv.getUint16(offset, true),
    dv.getUint8(offset + 2),
    dv.getUint8(offset + 3),
    dv.getUint8(offset + 4),
    dv.getUint8(offset + 5),
    dv.getUint8(offset + 6),
  );
}

/** `YYYYMMDD` for a LocalTime. */
export function dateKey(t: LocalTime): string {
  return `${pad(t.year, 4)}${pad(t.month)}${pad(t.day)}`;
}

export function makeDataView(buf: Uint8Array): DataView {
  return new DataView(buf.buffer, buf.byteOffset, buf.byteLength);
}

/** Read ASCII (latin1) bytes up to `maxLen` or the first NUL / 0xFF. */
export function readAscii(buf: Uint8Array, offset: number, maxLen: number): string {
  let s = '';
  const end = Math.min(buf.length, offset + maxLen);
  for (let i = offset; i < end; i++) {
    const c = buf[i]!;
    if (c === 0 || c === 0xff) break;
    s += String.fromCharCode(c);
  }
  return s;
}
