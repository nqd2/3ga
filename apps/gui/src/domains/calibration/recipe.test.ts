import { describe, expect, it } from 'vitest';
import {
  defaultConfig,
  defaultFloorFitPoints,
  defaultFloorPoints,
  defaultScalePoints,
  makeAlignmentRecipe,
} from './recipe';

describe('calibration recipe', () => {
  it('serializes floor and scale calibration for backend bake', () => {
    const recipe = makeAlignmentRecipe(2.5);

    expect(recipe.alignmentRecipe.floorPoints).toHaveLength(3);
    expect(recipe.alignmentRecipe.upAxis).toBe('y');
    expect(recipe.alignmentRecipe.scalePoints).toEqual([
      [0, 0, 0],
      [2, 0, 0],
    ]);
    expect(recipe.alignmentRecipe.scaleDistanceMeters).toBe(2.5);
  });

  it('serializes selected up axis', () => {
    const recipe = makeAlignmentRecipe(2.5, defaultFloorPoints, defaultScalePoints, 'z');

    expect(recipe.alignmentRecipe.upAxis).toBe('z');
  });

  it('serializes fitted floor points when fit mode is selected', () => {
    const recipe = makeAlignmentRecipe(
      2.5,
      defaultFloorPoints,
      defaultScalePoints,
      'y',
      'fit',
      defaultFloorFitPoints,
    );

    expect(recipe.alignmentRecipe.floorPoints).toBeUndefined();
    expect(recipe.alignmentRecipe.floorFitPoints).toHaveLength(4);
  });

  it('keeps IPC config path-based with smooth mesh export by default', () => {
    expect(defaultConfig.voxel.backend).toBe('cpu');
    expect(defaultConfig.mesh.mode).toBe('smooth');
  });
});
