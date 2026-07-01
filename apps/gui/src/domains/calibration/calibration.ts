import * as pc from 'playcanvas';
import { type Point3, type FloorMode, defaultFloorPoints } from './recipe';

export type Bounds = {
  min: { x: number; y: number; z: number };
  max: { x: number; y: number; z: number };
};

export type CalibrationPlane = { point: pc.Vec3; normal: pc.Vec3 };

export function calibrationPlane(points: [Point3, Point3, Point3]): CalibrationPlane {
  const a = toVec(points[0]);
  const b = toVec(points[1]);
  const c = toVec(points[2]);
  const normal = new pc.Vec3().cross(b.clone().sub(a), c.clone().sub(a));
  if (normal.lengthSq() < 1e-8) {
    return { point: a, normal: new pc.Vec3(0, 1, 0) };
  }
  return { point: a, normal: normal.normalize() };
}

export function previewFloorPoints(
  floorMode: FloorMode,
  floorPoints: [Point3, Point3, Point3],
  floorFitPoints: Point3[],
): [Point3, Point3, Point3] {
  if (floorMode === 'three-point') return floorPoints;
  for (let a = 0; a < floorFitPoints.length; a += 1) {
    for (let b = a + 1; b < floorFitPoints.length; b += 1) {
      for (let c = b + 1; c < floorFitPoints.length; c += 1) {
        const candidate: [Point3, Point3, Point3] = [
          floorFitPoints[a],
          floorFitPoints[b],
          floorFitPoints[c],
        ];
        if (calibrationPlane(candidate).normal.lengthSq() > 1e-8) return candidate;
      }
    }
  }
  return defaultFloorPoints;
}

export function intersectRayPlane(start: pc.Vec3, end: pc.Vec3, point: pc.Vec3, normal: pc.Vec3): pc.Vec3 {
  const direction = end.clone().sub(start);
  const denom = normal.dot(direction);
  if (Math.abs(denom) < 1e-6) return point.clone();
  const t = normal.dot(point.clone().sub(start)) / denom;
  return start.clone().add(direction.mulScalar(Math.max(t, 0)));
}

export function drawCalibrationOverlay(
  app: pc.Application,
  floorPoints: [Point3, Point3, Point3],
  scalePoints: [Point3, Point3],
  bounds?: Bounds,
) {
  const plane = calibrationPlane(floorPoints);
  const center = averagePoints(floorPoints);
  const basisU = planeBasisU(plane.normal, scalePoints);
  const basisV = new pc.Vec3().cross(plane.normal, basisU).normalize();
  const radius = overlayRadius(bounds, scalePoints);
  const gridColor = new pc.Color(0.2, 0.62, 0.48);
  const axisColor = new pc.Color(0.62, 0.9, 0.72);
  const markerColor = new pc.Color(1.0, 0.82, 0.35);
  const scaleColor = new pc.Color(0.45, 0.7, 1.0);
  const upColor = new pc.Color(1.0, 0.36, 0.26);

  for (let i = -5; i <= 5; i += 1) {
    const offset = (i / 5) * radius;
    app.drawLine(
      center.clone().add(basisV.clone().mulScalar(offset)).sub(basisU.clone().mulScalar(radius)),
      center.clone().add(basisV.clone().mulScalar(offset)).add(basisU.clone().mulScalar(radius)),
      i === 0 ? axisColor : gridColor,
      false,
    );
    app.drawLine(
      center.clone().add(basisU.clone().mulScalar(offset)).sub(basisV.clone().mulScalar(radius)),
      center.clone().add(basisU.clone().mulScalar(offset)).add(basisV.clone().mulScalar(radius)),
      i === 0 ? axisColor : gridColor,
      false,
    );
  }

  const upStart = center.clone();
  const upEnd = center.clone().add(plane.normal.clone().mulScalar(radius * 0.45));
  app.drawLine(upStart, upEnd, upColor, false);
  drawCross(app, upEnd, basisU, basisV, radius * 0.035, upColor);

  for (const point of floorPoints) {
    drawCross(app, toVec(point), basisU, basisV, radius * 0.025, markerColor);
  }
  const scaleA = toVec(scalePoints[0]);
  const scaleB = toVec(scalePoints[1]);
  app.drawLine(scaleA, scaleB, scaleColor, false);
  drawCross(app, scaleA, basisU, basisV, radius * 0.03, scaleColor);
  drawCross(app, scaleB, basisU, basisV, radius * 0.03, scaleColor);
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

export function planeBasisU(normal: pc.Vec3, scalePoints: [Point3, Point3]): pc.Vec3 {
  const rawScale = toVec(scalePoints[1]).sub(toVec(scalePoints[0]));
  const projected = rawScale.sub(normal.clone().mulScalar(rawScale.dot(normal)));
  if (projected.lengthSq() > 1e-8) return projected.normalize();
  const fallback = Math.abs(normal.y) < 0.9 ? new pc.Vec3(0, 1, 0) : new pc.Vec3(1, 0, 0);
  return fallback.sub(normal.clone().mulScalar(fallback.dot(normal))).normalize();
}

export function overlayRadius(bounds: Bounds | undefined, scalePoints: [Point3, Point3]): number {
  const scaleDistance = toVec(scalePoints[1]).sub(toVec(scalePoints[0])).length();
  if (!bounds) return Math.max(2, scaleDistance);
  const dx = bounds.max.x - bounds.min.x;
  const dy = bounds.max.y - bounds.min.y;
  const dz = bounds.max.z - bounds.min.z;
  return Math.max(1, Math.sqrt(dx * dx + dy * dy + dz * dz) * 0.55, scaleDistance);
}

export function averagePoints(points: readonly Point3[]): pc.Vec3 {
  const sum = points.reduce(
    (acc, point) => acc.add(toVec(point)),
    new pc.Vec3(0, 0, 0),
  );
  return sum.mulScalar(1 / points.length);
}

export function toVec(point: Point3): pc.Vec3 {
  return new pc.Vec3(point[0], point[1], point[2]);
}

export function roundPoint(point: pc.Vec3): Point3 {
  return [round3(point.x), round3(point.y), round3(point.z)];
}

export function round3(value: number): number {
  return Number(value.toFixed(3));
}
