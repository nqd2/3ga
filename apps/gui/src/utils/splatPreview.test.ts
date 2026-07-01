import { describe, expect, it } from 'vitest';
import { parseSplatColumns } from './splatPreview';

describe('splat preview parser', () => {
  it('decodes antimatter .splat rows into gaussian columns', () => {
    const bytes = new Uint8Array(32);
    const view = new DataView(bytes.buffer);
    view.setFloat32(0, 1, true);
    view.setFloat32(4, 2, true);
    view.setFloat32(8, 3, true);
    view.setFloat32(12, 1, true);
    view.setFloat32(16, 2, true);
    view.setFloat32(20, 4, true);
    bytes.set([255, 128, 0, 128, 255, 128, 128, 128], 24);

    const parsed = parseSplatColumns(bytes);

    expect(parsed.count).toBe(1);
    expect(parsed.columns.x[0]).toBe(1);
    expect(parsed.columns.y[0]).toBe(2);
    expect(parsed.columns.z[0]).toBe(3);
    expect(parsed.columns.scale_0[0]).toBeCloseTo(0);
    expect(parsed.columns.scale_1[0]).toBeCloseTo(Math.log(2));
    expect(parsed.columns.scale_2[0]).toBeCloseTo(Math.log(4));
    expect(parsed.columns.opacity[0]).toBeCloseTo(Math.log((128 / 255) / (1 - 128 / 255)));
    expect(parsed.columns.rot_0[0]).toBeGreaterThan(0);
  });

  it('rejects invalid byte lengths and non-positive scales', () => {
    expect(() => parseSplatColumns(new Uint8Array(31))).toThrow(/byte length/);

    const bytes = new Uint8Array(32);
    const view = new DataView(bytes.buffer);
    view.setFloat32(12, 1, true);
    view.setFloat32(16, 0, true);
    view.setFloat32(20, 1, true);

    expect(() => parseSplatColumns(bytes)).toThrow(/scale/);
  });
});
