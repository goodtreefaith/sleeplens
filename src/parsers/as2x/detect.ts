import type { As2xDetection, Nightfile } from './types';

const DAT_NAME = /^(\d{4})(\d{2})(\d{2})\.dat$/i;

/**
 * Recognise the AS2X card layout from file names alone (paths with `/` or `\`,
 * as `webkitRelativePath` or a directory walk would give):
 * `<serial><YYYYMM>/<YYYYMMDD>.dat` plus `[<letters>]<serial>.bkp`.
 */
export function detectAs2x(fileNames: readonly string[]): As2xDetection {
  const nights: Nightfile[] = [];
  const serials = new Map<string, number>();
  const months = new Set<string>();
  let bkp: string | null = null;
  let bkpSerial: string | null = null;

  for (const path of fileNames) {
    const parts = path.split(/[\\/]/).filter(Boolean);
    const name = parts[parts.length - 1];
    if (!name) continue;

    if (/\.bkp$/i.test(name) && parts.length <= 2) {
      const base = name.slice(0, -4);
      if (base && (!bkp || parts.length < bkp.split(/[\\/]/).filter(Boolean).length)) {
        bkp = path;
        bkpSerial = base;
      }
      continue;
    }

    const m = DAT_NAME.exec(name);
    const folder = parts[parts.length - 2];
    if (!m || !folder) continue;
    const yyyymm = `${m[1]}${m[2]}`;
    if (!folder.endsWith(yyyymm) || folder.length === yyyymm.length) continue;
    const serial = folder.slice(0, folder.length - yyyymm.length);
    serials.set(serial, (serials.get(serial) ?? 0) + 1);
    months.add(yyyymm);
    nights.push({ date: `${m[1]}${m[2]}${m[3]}`, path, folder });
  }

  let serial: string | null = null;
  // The folder serial is authoritative. The .bkp base name must END with it:
  // real cards prefix it with a few letters (for example `ABC12345.bkp` next to `12345YYYYMM/`).
  if (bkpSerial) {
    const hit = [...serials.keys()]
      .filter((s) => bkpSerial!.endsWith(s))
      .sort((a, b) => (serials.get(b) ?? 0) - (serials.get(a) ?? 0))[0];
    if (hit) serial = hit;
  }
  if (!serial && serials.size > 0) serial = [...serials.entries()].sort((a, b) => b[1] - a[1])[0]![0];

  const kept = serial ? nights.filter((n) => n.folder.startsWith(serial)) : [];
  kept.sort((a, b) => a.date.localeCompare(b.date));
  return {
    detected: kept.length > 0,
    serial,
    nights: kept,
    bkp: serial && bkpSerial && bkpSerial.endsWith(serial) ? bkp : null,
    months: [...months].sort(),
  };
}
