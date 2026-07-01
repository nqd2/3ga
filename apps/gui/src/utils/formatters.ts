import { boundsPointToPoint3, type Bounds } from '../domains/calibration';
import { type Point3 } from '../domains/calibration';

export function formatBytes(value: number | string | null | undefined) {
  const bytes = Number(value);
  if (!Number.isFinite(bytes) || bytes <= 0) return 'N/A';
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

export function formatNumber(value: number | string | null | undefined) {
  const number = Number(value);
  if (!Number.isFinite(number)) return 'N/A';
  return number.toLocaleString();
}

export function formatRatio(value: number | string | null | undefined) {
  const ratio = Number(value);
  if (!Number.isFinite(ratio) || ratio <= 0) return 'N/A';
  return `${ratio.toFixed(1)}x`;
}

export function formatBounds(bounds?: Bounds | null) {
  if (!bounds) return 'N/A';
  const min = boundsPointToPoint3(bounds.min);
  const max = boundsPointToPoint3(bounds.max);
  const formatPoint = (point: Point3) => point.map((value) => value.toFixed(2)).join(', ');
  return `min ${formatPoint(min)} / max ${formatPoint(max)}`;
}
