/** Usage: tsx tools/make-synthetic-as2x.ts <outDir>   (npm run make-fixture) */
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { buildSyntheticCard } from './lib/synthetic';

const outDir = process.argv[2];
if (!outDir) {
  console.error('Usage: tsx tools/make-synthetic-as2x.ts <outDir>');
  process.exit(1);
}
if (resolve(outDir).startsWith('/Volumes/')) {
  console.error('Refusing to write to /Volumes: this tool is for fixtures only.');
  process.exit(1);
}

let total = 0;
for (const [rel, bytes] of buildSyntheticCard()) {
  const p = join(outDir, rel);
  mkdirSync(dirname(p), { recursive: true });
  writeFileSync(p, bytes);
  total += bytes.length;
  console.log(`${rel}  ${bytes.length} bytes`);
}
console.log(`Total ${(total / 1024).toFixed(0)} KiB in ${outDir}`);
