import { describe, expect, it } from 'vitest';
import {
  defaultGeometryProfile,
  defaultScalePoints,
  geometryProfileStorageVersion,
  makeAlignmentRecipe,
  makeEditRecipe,
  makeProcessConfig,
  storedGeometryProfileOrDefault,
} from './index';

describe('calibration profile defaults', () => {
  it('defaults to object prop baking without carve, navmesh, or cluster filtering', () => {
    const config = makeProcessConfig(defaultScalePoints, 2);
    const recipe = makeAlignmentRecipe(2, defaultScalePoints);

    expect(defaultGeometryProfile).toBe('object-prop');
    expect(config.voxel).toEqual({ backend: 'cpu', size: 0.05, opacityThreshold: 0.1 });
    expect(config.voxelFill).toEqual({ mode: 'none', dilationSize: 0 });
    expect(config.voxelCarve.enabled).toBe(false);
    expect(config.navmesh.enabled).toBe(false);
    expect(config.navmesh).toMatchObject({
      agentHeight: 1.6,
      agentRadius: 0.2,
      maxSlopeDegrees: 45,
      cellSize: 0.1,
      cellHeight: 0.05,
      walkableClimb: 0.25,
      minRegionSize: 4,
      mergeRegionSize: 12,
    });
    expect(recipe.editRecipe.operations).toEqual([]);
  });

  it('does not inject filterCluster for any bake profile', () => {
    expect(makeEditRecipe(defaultScalePoints, 2, 'object-prop').operations).toEqual([]);
    expect(makeEditRecipe(defaultScalePoints, 2, 'interior-room').operations).toEqual([]);
    expect(makeEditRecipe(defaultScalePoints, 2, 'outdoor-terrain').operations).toEqual([]);
  });

  it('keeps room and terrain process profile behavior explicit', () => {
    const room = makeProcessConfig(defaultScalePoints, 2, 'interior-room');
    const terrain = makeProcessConfig(defaultScalePoints, 2, 'outdoor-terrain');

    expect(room.voxelFill.mode).toBe('exterior-fill');
    expect(room.voxelCarve.enabled).toBe(true);
    expect(room.navmesh.enabled).toBe(true);
    expect(terrain.voxelFill.mode).toBe('floor-fill');
    expect(terrain.voxelCarve.enabled).toBe(false);
    expect(terrain.navmesh.enabled).toBe(false);
  });

  it('migrates stale stored geometry profile to object prop once', () => {
    expect(storedGeometryProfileOrDefault('interior-room', undefined)).toBe('object-prop');
    expect(storedGeometryProfileOrDefault('interior-room', 1)).toBe('object-prop');
    expect(storedGeometryProfileOrDefault('interior-room', geometryProfileStorageVersion))
      .toBe('interior-room');
    expect(storedGeometryProfileOrDefault('bad-profile', geometryProfileStorageVersion))
      .toBe('object-prop');
  });
});
