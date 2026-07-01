import * as pc from 'playcanvas';

export type Point3 = [number, number, number];
export type UpAxis = 'x' | 'y' | 'z' | 'neg-x' | 'neg-y' | 'neg-z';
export type PickMode = 'scale0' | 'scale1';
export type GeometryProfile = 'object-prop' | 'interior-room' | 'outdoor-terrain';

export const defaultGeometryProfile: GeometryProfile = 'object-prop';
export const geometryProfileStorageKey = 'ag_bake_geometry_profile';
export const geometryProfileStorageVersionKey = 'ag_bake_geometry_profile_version';
export const geometryProfileStorageVersion = 2;

const geometryProfiles = new Set<GeometryProfile>([
  'object-prop',
  'interior-room',
  'outdoor-terrain',
]);

export function normalizeGeometryProfile(value: unknown): GeometryProfile {
  return typeof value === 'string' && geometryProfiles.has(value as GeometryProfile)
    ? value as GeometryProfile
    : defaultGeometryProfile;
}

export function storedGeometryProfileOrDefault(
  value: unknown,
  version: unknown,
): GeometryProfile {
  return version === geometryProfileStorageVersion
    ? normalizeGeometryProfile(value)
    : defaultGeometryProfile;
}

export const defaultScalePoints: [Point3, Point3] = [
  [0, 0, 0],
  [2, 0, 0],
];

export function upAxisVector(upAxis: UpAxis): Point3 {
  switch (upAxis) {
    case 'x': return [1, 0, 0];
    case 'neg-x': return [-1, 0, 0];
    case 'z': return [0, 0, 1];
    case 'neg-z': return [0, 0, -1];
    case 'neg-y': return [0, -1, 0];
    case 'y':
    default: return [0, 1, 0];
  }
}

export function collisionSeedForUpAxis(upAxis: UpAxis, height = 1.0): Point3 {
  const up = upAxisVector(upAxis);
  return [up[0] * height, up[1] * height, up[2] * height];
}

export function makeProcessConfig(
  scalePoints: [Point3, Point3],
  distance: number,
  profile: GeometryProfile = defaultGeometryProfile,
  upAxis: UpAxis = 'y',
) {
  const upVec = upAxisVector(upAxis);
  const midX = (scalePoints[0][0] + scalePoints[1][0]) / 2;
  const midY = (scalePoints[0][1] + scalePoints[1][1]) / 2;
  const midZ = (scalePoints[0][2] + scalePoints[1][2]) / 2;
  
  let unitScale = pointDistance(scalePoints[0], scalePoints[1]) / distance;
  if (!Number.isFinite(unitScale) || unitScale === 0) {
    unitScale = 1.0;
  }
  // Offset by 1 real-world meter
  const seedPos: Point3 = [
    midX + upVec[0] * unitScale,
    midY + upVec[1] * unitScale,
    midZ + upVec[2] * unitScale,
  ];

  if (profile === 'outdoor-terrain') {
    return {
      voxel: { backend: 'cpu', size: 0.05, opacityThreshold: 0.1 },
      voxelFill: { mode: 'floor-fill', dilationSize: 0 },
      voxelCarve: { enabled: false, agentHeight: 1.6, agentRadius: 0.2, seedPos },
      navmesh: { ...defaultNavmeshConfig, enabled: false },
      mesh: { mode: 'smooth' },
    };
  }
  if (profile === 'interior-room') {
    return {
      voxel: { backend: 'cpu', size: 0.05, opacityThreshold: 0.1 },
      voxelFill: { mode: 'exterior-fill', dilationSize: 1.6 },
      voxelCarve: { enabled: true, agentHeight: 1.6, agentRadius: 0.2, seedPos },
      navmesh: { ...defaultNavmeshConfig, enabled: true },
      mesh: { mode: 'smooth' },
    };
  }
  return {
    voxel: { backend: 'cpu', size: 0.05, opacityThreshold: 0.1 },
    voxelFill: { mode: 'none', dilationSize: 0 },
    voxelCarve: { enabled: false, agentHeight: 1.6, agentRadius: 0.2, seedPos },
    navmesh: { ...defaultNavmeshConfig, enabled: false },
    mesh: { mode: 'smooth' },
  };
}

export function makeEditRecipe(
  scalePoints: [Point3, Point3],
  distance: number,
  profile: GeometryProfile = defaultGeometryProfile,
  upAxis: UpAxis = 'y',
) {
  return {
    operations: [],
  };
}

const defaultNavmeshConfig = {
  enabled: true,
  agentHeight: 1.6,
  agentRadius: 0.2,
  maxSlopeDegrees: 45,
  cellSize: 0.1,
  cellHeight: 0.05,
  walkableClimb: 0.25,
  minRegionSize: 4,
  mergeRegionSize: 12,
};

export function makeAlignmentRecipe(
  distance: number,
  scalePoints: [Point3, Point3] = defaultScalePoints,
  upAxis: UpAxis = 'y',
  geometryProfile: GeometryProfile = defaultGeometryProfile,
) {
  return {
    alignmentRecipe: {
      upAxis,
      floorNormal: upAxisVector(upAxis),
      scalePoints,
      scaleDistanceMeters: distance,
      origin: [0, 0, 0] as Point3,
    },
    editRecipe: makeEditRecipe(scalePoints, distance, geometryProfile, upAxis),
  };
}

export function isFinitePoint(point: Point3) {
  return point.every((value) => Number.isFinite(value));
}

export function pointDistance(a: Point3, b: Point3) {
  const dx = b[0] - a[0];
  const dy = b[1] - a[1];
  const dz = b[2] - a[2];
  return Math.sqrt(dx * dx + dy * dy + dz * dz);
}

export function hasValidScale(points: [Point3, Point3]) {
  return points.every(isFinitePoint) && pointDistance(points[0], points[1]) > 1e-8;
}

export type Bounds = {
  min: Point3 | { x: number; y: number; z: number };
  max: Point3 | { x: number; y: number; z: number };
};

export function toVec(point: Point3): pc.Vec3 {
  return new pc.Vec3(point[0], point[1], point[2]);
}

export function boundsPointToPoint3(point: Bounds['min']): Point3 {
  return Array.isArray(point) ? point : [point.x, point.y, point.z];
}

export function overlayRadius(bounds: Bounds | undefined, scalePoints: [Point3, Point3]): number {
  const scaleDistance = toVec(scalePoints[1]).sub(toVec(scalePoints[0])).length();
  if (!bounds) return Math.max(2, scaleDistance);
  const min = boundsPointToPoint3(bounds.min);
  const max = boundsPointToPoint3(bounds.max);
  const dx = max[0] - min[0];
  const dy = max[1] - min[1];
  const dz = max[2] - min[2];
  return Math.max(1, Math.sqrt(dx * dx + dy * dy + dz * dz) * 0.55, scaleDistance);
}

export function drawCalibrationOverlay(
  app: pc.Application,
  scalePoints: [Point3, Point3],
  bounds?: Bounds,
  transformMatrix?: pc.Mat4,
  upAxis: UpAxis = 'y',
) {
  const center = new pc.Vec3(0, 0, 0);
  const upVec = toVec(upAxisVector(upAxis));
  
  // Create an arbitrary orthogonal basis
  const fallback = Math.abs(upVec.y) < 0.9 ? new pc.Vec3(0, 1, 0) : new pc.Vec3(1, 0, 0);
  const basisU = fallback.sub(upVec.clone().mulScalar(fallback.dot(upVec))).normalize();
  const basisV = new pc.Vec3().cross(upVec, basisU).normalize();
  
  const radius = overlayRadius(bounds, scalePoints);
  const gridColor = new pc.Color(0.3, 0.3, 0.3);
  const axisColor = new pc.Color(0.82, 0.07, 0.07);
  const scaleColor = new pc.Color(0.45, 0.7, 1.0);
  const upColor = new pc.Color(0.82, 0.07, 0.07);

  const txPoint = (p: pc.Vec3) => transformMatrix ? transformMatrix.transformPoint(p, new pc.Vec3()) : p;
  const txVec = (v: pc.Vec3) => transformMatrix ? transformMatrix.transformVector(v, new pc.Vec3()) : v;

  const worldCenter = txPoint(center);
  const worldBasisU = txVec(basisU);
  const worldBasisV = txVec(basisV);
  const worldNormal = txVec(upVec);

  for (let i = -5; i <= 5; i += 1) {
    const offset = (i / 5) * radius;
    app.drawLine(
      worldCenter.clone().add(worldBasisV.clone().mulScalar(offset)).sub(worldBasisU.clone().mulScalar(radius)),
      worldCenter.clone().add(worldBasisV.clone().mulScalar(offset)).add(worldBasisU.clone().mulScalar(radius)),
      i === 0 ? axisColor : gridColor,
      false,
    );
    app.drawLine(
      worldCenter.clone().add(worldBasisU.clone().mulScalar(offset)).sub(worldBasisV.clone().mulScalar(radius)),
      worldCenter.clone().add(worldBasisU.clone().mulScalar(offset)).add(worldBasisV.clone().mulScalar(radius)),
      i === 0 ? axisColor : gridColor,
      false,
    );
  }

  const upStart = worldCenter.clone();
  const upEnd = worldCenter.clone().add(worldNormal.clone().mulScalar(radius * 0.45));
  app.drawLine(upStart, upEnd, upColor, false);
  drawCross(app, upEnd, worldBasisU, worldBasisV, radius * 0.035, upColor);

  const scaleA = txPoint(toVec(scalePoints[0]));
  const scaleB = txPoint(toVec(scalePoints[1]));
  app.drawLine(scaleA, scaleB, scaleColor, false);
  drawCross(app, scaleA, worldBasisU, worldBasisV, radius * 0.03, scaleColor);
  drawCross(app, scaleB, worldBasisU, worldBasisV, radius * 0.03, scaleColor);
}

export function drawCross(
  app: pc.Application,
  point: pc.Vec3,
  basisU: pc.Vec3,
  basisV: pc.Vec3,
  size: number,
  color: pc.Color,
) {
  app.drawLine(
    point.clone().sub(basisU.clone().mulScalar(size)),
    point.clone().add(basisU.clone().mulScalar(size)),
    color,
    false,
  );
  app.drawLine(
    point.clone().sub(basisV.clone().mulScalar(size)),
    point.clone().add(basisV.clone().mulScalar(size)),
    color,
    false,
  );
}

export function round3(value: number): number {
  return Number(value.toFixed(3));
}

export function roundPoint(point: pc.Vec3): Point3 {
  return [round3(point.x), round3(point.y), round3(point.z)];
}
export function intersectRayPlane(start: pc.Vec3, end: pc.Vec3, point: pc.Vec3, normal: pc.Vec3): pc.Vec3 {
  const direction = end.clone().sub(start);
  const denom = normal.dot(direction);
  if (Math.abs(denom) < 1e-6) return point.clone();
  const t = normal.dot(point.clone().sub(start)) / denom;
  return start.clone().add(direction.mulScalar(Math.max(t, 0)));
}
