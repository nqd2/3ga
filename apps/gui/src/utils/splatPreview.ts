const BYTES_PER_SPLAT = 32;
const SH_C0 = 0.282_094_8;

export type SplatColumnName =
  | 'x'
  | 'y'
  | 'z'
  | 'scale_0'
  | 'scale_1'
  | 'scale_2'
  | 'opacity'
  | 'f_dc_0'
  | 'f_dc_1'
  | 'f_dc_2'
  | 'rot_0'
  | 'rot_1'
  | 'rot_2'
  | 'rot_3';

export type ParsedSplatColumns = {
  count: number;
  columns: Record<SplatColumnName, Float32Array>;
};

const columnNames: SplatColumnName[] = [
  'x',
  'y',
  'z',
  'scale_0',
  'scale_1',
  'scale_2',
  'opacity',
  'f_dc_0',
  'f_dc_1',
  'f_dc_2',
  'rot_0',
  'rot_1',
  'rot_2',
  'rot_3',
];

export function parseSplatColumns(input: Uint8Array | ArrayBuffer): ParsedSplatColumns {
  const bytes = input instanceof Uint8Array ? input : new Uint8Array(input);
  if (bytes.length === 0 || bytes.length % BYTES_PER_SPLAT !== 0) {
    throw new Error(`Invalid .splat byte length ${bytes.length}`);
  }

  const count = bytes.length / BYTES_PER_SPLAT;
  const columns = Object.fromEntries(
    columnNames.map((name) => [name, new Float32Array(count)]),
  ) as Record<SplatColumnName, Float32Array>;
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);

  for (let i = 0; i < count; i += 1) {
    const offset = i * BYTES_PER_SPLAT;
    columns.x[i] = view.getFloat32(offset + 0, true);
    columns.y[i] = view.getFloat32(offset + 4, true);
    columns.z[i] = view.getFloat32(offset + 8, true);

    const scaleX = view.getFloat32(offset + 12, true);
    const scaleY = view.getFloat32(offset + 16, true);
    const scaleZ = view.getFloat32(offset + 20, true);
    if (scaleX <= 0 || scaleY <= 0 || scaleZ <= 0) {
      throw new Error('.splat linear scale must be positive');
    }
    columns.scale_0[i] = Math.log(scaleX);
    columns.scale_1[i] = Math.log(scaleY);
    columns.scale_2[i] = Math.log(scaleZ);

    columns.f_dc_0[i] = dcFromU8(bytes[offset + 24]);
    columns.f_dc_1[i] = dcFromU8(bytes[offset + 25]);
    columns.f_dc_2[i] = dcFromU8(bytes[offset + 26]);

    const alpha = Math.max(1e-6, Math.min(1 - 1e-6, bytes[offset + 27] / 255));
    columns.opacity[i] = Math.log(alpha / (1 - alpha));

    const q = normalizeQuaternion([
      byteToQuat(bytes[offset + 28]),
      byteToQuat(bytes[offset + 29]),
      byteToQuat(bytes[offset + 30]),
      byteToQuat(bytes[offset + 31]),
    ]);
    columns.rot_0[i] = q[0];
    columns.rot_1[i] = q[1];
    columns.rot_2[i] = q[2];
    columns.rot_3[i] = q[3];
  }

  return { count, columns };
}

export function splatColumnsToPlyElements(parsed: ParsedSplatColumns) {
  return [{
    name: 'vertex',
    count: parsed.count,
    properties: columnNames.map((name) => ({
      name,
      type: 'float',
      byteSize: 4,
      storage: parsed.columns[name],
    })),
  }];
}

function dcFromU8(value: number) {
  return ((value / 255) - 0.5) / SH_C0;
}

function byteToQuat(value: number) {
  return (value / 255) * 2 - 1;
}

function normalizeQuaternion(value: [number, number, number, number]) {
  const length = Math.hypot(value[0], value[1], value[2], value[3]);
  if (length <= 1e-8) return [1, 0, 0, 0] as const;
  return [
    value[0] / length,
    value[1] / length,
    value[2] / length,
    value[3] / length,
  ] as const;
}
