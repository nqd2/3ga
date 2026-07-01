import { describe, expect, it } from 'vitest';
import {
  defaultSceneTransform,
  isDefaultSceneTransform,
  updateTransformAxis,
} from './sceneTransform';

describe('scene transform helpers', () => {
  it('updates one XYZ axis without mutating original transform', () => {
    const next = updateTransformAxis(defaultSceneTransform, 'rotationEulerDeg', 1, 45);

    expect(defaultSceneTransform.rotationEulerDeg).toEqual([0, 0, 0]);
    expect(next.rotationEulerDeg).toEqual([0, 45, 0]);
    expect(next.position).toEqual([0, 0, 0]);
  });

  it('detects dirty transform state', () => {
    expect(isDefaultSceneTransform(defaultSceneTransform)).toBe(true);
    expect(isDefaultSceneTransform({
      position: [0, 0.1, 0],
      rotationEulerDeg: [0, 0, 0],
    })).toBe(false);
  });
});
