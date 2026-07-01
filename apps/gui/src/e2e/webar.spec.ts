import { expect, test } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, statSync } from 'node:fs';
import path from 'node:path';
import { inflateSync } from 'node:zlib';
import { serveDirectory } from './server';

test('exported WebAR bundle page loads all declared assets', async ({ page }) => {
  const repoRoot = path.resolve(process.cwd(), '../..');
  const outDir = path.join(repoRoot, 'target/e2e/playwright-webar');
  execFileSync(
    'cargo',
    [
      'run',
      '-p',
      'augmented-gaussian-cli',
      '--',
      'process',
      'tests/fixtures/minimal.splat',
      '--out',
      outDir,
      '--config',
      'tests/config/basic.json',
    ],
    { cwd: repoRoot, stdio: 'pipe' },
  );

  expect(existsSync(path.join(outDir, 'index.html'))).toBe(true);
  expect(existsSync(path.join(outDir, 'assets/js/playcanvas.min.js'))).toBe(true);
  expect(existsSync(path.join(outDir, 'webar.zip'))).toBe(true);
  expect(statSync(path.join(outDir, 'webar.zip')).size).toBeGreaterThan(0);
  const manifest = JSON.parse(readFileSync(path.join(outDir, 'manifest.json'), 'utf8'));
  const declaredAssetCount = [
    manifest.artifacts.scene,
    manifest.artifacts.collisionMeshJson,
    manifest.artifacts.occlusionGlb,
    manifest.artifacts.navmeshGlb,
    manifest.artifacts.navmeshBin,
  ].filter(Boolean).length;

  const { server, url } = await serveDirectory(outDir);
  try {
    await page.goto(`${url}/index.html`);
    await expect(page.locator('body')).toHaveAttribute('data-loaded', 'true');
    await expect(page.locator('body')).toHaveAttribute('data-camera-ready', 'true');
    await expect(page.locator('#assets li')).toHaveCount(declaredAssetCount);
    const screenshot = await page.locator('#viewport').screenshot();
    expect(pngHasVisibleVariation(screenshot)).toBe(true);
  } finally {
    await new Promise<void>((resolve) => server.close(() => resolve()));
  }
});

function pngHasVisibleVariation(png: Buffer): boolean {
  let offset = 8;
  let width = 0;
  let height = 0;
  let bitDepth = 0;
  let colorType = 0;
  const idat: Buffer[] = [];
  while (offset < png.length) {
    const length = png.readUInt32BE(offset);
    const type = png.toString('ascii', offset + 4, offset + 8);
    const dataStart = offset + 8;
    const dataEnd = dataStart + length;
    if (type === 'IHDR') {
      width = png.readUInt32BE(dataStart);
      height = png.readUInt32BE(dataStart + 4);
      bitDepth = png[dataStart + 8];
      colorType = png[dataStart + 9];
    }
    if (type === 'IDAT') idat.push(png.subarray(dataStart, dataEnd));
    if (type === 'IEND') break;
    offset = dataEnd + 4;
  }
  if (bitDepth !== 8 || (colorType !== 2 && colorType !== 6)) return false;
  const bytesPerPixel = colorType === 6 ? 4 : 3;
  const stride = width * bytesPerPixel;
  const data = inflateSync(Buffer.concat(idat));
  let source = 0;
  let previous = Buffer.alloc(stride);
  let first: number[] | null = null;
  let visible = false;
  let varied = false;
  for (let y = 0; y < height; y += 1) {
    const filter = data[source];
    source += 1;
    const row = Buffer.from(data.subarray(source, source + stride));
    source += stride;
    unfilter(row, previous, bytesPerPixel, filter);
    const step = Math.max(bytesPerPixel, Math.floor(stride / 32 / bytesPerPixel) * bytesPerPixel);
    for (let x = 0; x < stride; x += step) {
      const color = [row[x], row[x + 1], row[x + 2], colorType === 6 ? row[x + 3] : 255];
      if (color[3] > 0 && color[0] + color[1] + color[2] > 0) visible = true;
      if (!first) first = color;
      if (first && color.some((value, index) => Math.abs(value - first![index]) > 5)) {
        varied = true;
      }
    }
    previous = row;
  }
  return visible && varied;
}

function unfilter(row: Buffer, previous: Buffer, bytesPerPixel: number, filter: number) {
  for (let i = 0; i < row.length; i += 1) {
    const left = i >= bytesPerPixel ? row[i - bytesPerPixel] : 0;
    const up = previous[i] ?? 0;
    const upLeft = i >= bytesPerPixel ? previous[i - bytesPerPixel] : 0;
    if (filter === 1) row[i] = (row[i] + left) & 0xff;
    if (filter === 2) row[i] = (row[i] + up) & 0xff;
    if (filter === 3) row[i] = (row[i] + Math.floor((left + up) / 2)) & 0xff;
    if (filter === 4) row[i] = (row[i] + paeth(left, up, upLeft)) & 0xff;
  }
}

function paeth(left: number, up: number, upLeft: number) {
  const p = left + up - upLeft;
  const pa = Math.abs(p - left);
  const pb = Math.abs(p - up);
  const pc = Math.abs(p - upLeft);
  if (pa <= pb && pa <= pc) return left;
  if (pb <= pc) return up;
  return upLeft;
}
