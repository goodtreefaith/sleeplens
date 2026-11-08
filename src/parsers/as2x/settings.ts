import type { SettingsSnapshot } from './types';
import { makeDataView, readTimestamp } from './time';

export const BLOCK_SIZE = 128;

/** Parse a 128-byte `C5 C5` settings block at `offset`. Returns null if the magic is wrong. */
export function parseSettingsBlock(buf: Uint8Array, offset: number): SettingsSnapshot | null {
  if (offset + BLOCK_SIZE > buf.length) return null;
  if (buf[offset] !== 0xc5 || buf[offset + 1] !== 0xc5) return null;
  const dv = makeDataView(buf);
  const pressureFields: number[] = [];
  for (const rel of [3, 5, 7, 9]) pressureFields.push(dv.getUint16(offset + rel, true));
  return {
    time: readTimestamp(dv, offset + 0x21),
    pressureFields,
    humidifierRaw: buf[offset + 0x2e]!,
    raw: buf.slice(offset, offset + BLOCK_SIZE),
  };
}
