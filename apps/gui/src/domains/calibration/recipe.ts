export const defaultConfig = {
voxel: { backend: 'cpu', size: 0.1, opacityThreshold: 0.1 },
voxelFill: { mode: 'none', dilationSize: 0 },
voxelCarve: { enabled: false, agentHeight: 1.6, agentRadius: 0.2, seedPos: [0, 0, 0] },
mesh: { mode: 'smooth' },
};

export type Point3 = [number, number, number];
export type UpAxis = 'x' | 'y' | 'z' | 'neg-x' | 'neg-y' | 'neg-z';
export type FloorMode = 'three-point' | 'fit';

export const defaultFloorPoints: [Point3, Point3, Point3] = [
[0, 0, 0],
[1, 0, 0],
[0, 0, 1],
];

export const defaultScalePoints: [Point3, Point3] = [
[0, 0, 0],
[2, 0, 0],
];

export const defaultFloorFitPoints: Point3[] = [
[0, 0, 0],
[1, 0, 0],
[0, 0, 1],
[1, 0, 1],
];

export function makeAlignmentRecipe(
distance: number,
floorPoints: [Point3, Point3, Point3] = defaultFloorPoints,
scalePoints: [Point3, Point3] = defaultScalePoints,
upAxis: UpAxis = 'y',
floorMode: FloorMode = 'three-point',
floorFitPoints: Point3[] = defaultFloorFitPoints,
) {
const fitPoints = floorFitPoints.length >= 3 ? floorFitPoints : defaultFloorFitPoints;
const floor = floorMode === 'fit'
? { floorFitPoints: fitPoints }
: { floorPoints };

return {
alignmentRecipe: {
...floor,
upAxis,
scalePoints,
scaleDistanceMeters: distance,
},
editRecipe: { operations: [] },
};
}
