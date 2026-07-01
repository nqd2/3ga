import { type Bounds } from '../../domains/calibration';

export type SceneTransform = {
  position: [number, number, number];
  rotationEulerDeg: [number, number, number];
};

export type EditorSceneState = {
  sourcePath: string | null;
  visible: boolean;
  deleted: boolean;
  transform: SceneTransform;
  splatCount: number;
  bounds: Bounds | null;
};

export type EditorSourceExport = {
  path: string;
  originalPath: string;
  bytes: number;
  splatCount: number;
  bounds: Bounds;
};

export const defaultSceneTransform: SceneTransform = {
  position: [0, 0, 0],
  rotationEulerDeg: [0, 0, 0],
};

export function updateTransformAxis(
  transform: SceneTransform,
  key: keyof SceneTransform,
  index: number,
  value: number,
): SceneTransform {
  const next = [...transform[key]] as [number, number, number];
  next[index] = Number.isFinite(value) ? value : 0;
  return { ...transform, [key]: next };
}

export function isDefaultSceneTransform(transform: SceneTransform) {
  return transform.position.every((value) => value === 0)
    && transform.rotationEulerDeg.every((value) => value === 0);
}
