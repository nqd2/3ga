import * as pc from 'playcanvas';
import { CameraController } from './CameraController';
import {
  drawCalibrationOverlay,
  intersectRayPlane,
  roundPoint,
  upAxisVector,
  type Bounds,
} from '../domains/calibration';
import { type Point3, type UpAxis } from '../domains/calibration';
import { parseSplatColumns, splatColumnsToPlyElements } from '../utils/splatPreview';
import { defaultSceneTransform, type SceneTransform } from '../domains/editor/sceneTransform';
import { type CameraMode } from './CameraController';

export type ViewerLoadProgress = {
  loaded: number;
  total: number | null;
  percent: number | null;
};

export type ViewerSourceSummary = {
  splatCount: number;
  bounds: Bounds;
};

export type LoadSplatOptions = {
  bounds?: Bounds;
  onProgress?: (progress: ViewerLoadProgress) => void;
  onReady?: (summary: ViewerSourceSummary) => void;
  onError?: (error: Error) => void;
};

export class PlayCanvasViewer {
  private app: pc.Application;
  private camera: pc.Entity;
  private controller: CameraController;
  private canvas: HTMLCanvasElement;
  private gsplatEntity: pc.Entity | null = null;
  private gsplatAsset: pc.Asset | null = null;
  private gsplatResource: any = null;
  private loadToken = 0;
  private loadAbort: AbortController | null = null;

  private scalePoints: [Point3, Point3] = [[0, 0, 0], [0, 0, 0]];
  private bounds?: Bounds;
  private upAxis: UpAxis = 'y';
  private sceneTransform: SceneTransform = defaultSceneTransform;
  private alignmentRotation = new pc.Quat();

  constructor(canvas: HTMLCanvasElement, bounds?: Bounds, sourceUrl?: string | null) {
    this.canvas = canvas;
    this.bounds = bounds;
    this.app = new pc.Application(canvas, {
      graphicsDeviceOptions: { alpha: false },
    });
    this.app.setCanvasFillMode(pc.FILLMODE_FILL_WINDOW);
    this.app.setCanvasResolution(pc.RESOLUTION_AUTO);

    // Setup camera
    this.camera = new pc.Entity('camera');
    this.camera.addComponent('camera', { clearColor: new pc.Color(0.04, 0.05, 0.055) });
    this.app.root.addChild(this.camera);

    // Setup light
    const light = new pc.Entity('light');
    light.addComponent('light', { type: 'directional', intensity: 1.2 });
    light.setEulerAngles(45, 35, 0);
    this.app.root.addChild(light);

    // Controller
    this.controller = new CameraController(this.camera, canvas);
    this.controller.fitBounds(bounds);

    this.app.on('update', this.onUpdate);

    if (sourceUrl) {
      this.loadSplat(sourceUrl, { bounds });
    }

    this.app.start();
  }

  private onUpdate = (dt: number) => {
    this.controller.update(dt);
    const transformMatrix = this.alignmentPreviewMatrix();
    drawCalibrationOverlay(
      this.app,
      this.scalePoints,
      this.bounds,
      transformMatrix,
      this.upAxis
    );
  };

  public updateCalibration(
    scalePoints: [Point3, Point3],
    bounds?: Bounds,
    upAxis?: UpAxis,
  ) {
    this.scalePoints = scalePoints;
    this.bounds = bounds;
    if (upAxis) {
      this.upAxis = upAxis;
    }
    this.updateAlignmentTransform();
  }

  private updateAlignmentTransform() {
    this.alignmentRotation.copy(pc.Quat.IDENTITY);
    if (!this.gsplatEntity) return;

    const manualRotation = sceneTransformRotation(this.sceneTransform);
    const finalRotation = new pc.Quat().mul2(this.alignmentRotation, manualRotation);
    const manualPosition = sceneTransformPosition(this.sceneTransform);
    const finalPosition = this.alignmentRotation.transformVector(manualPosition, new pc.Vec3());
    this.gsplatEntity.setPosition(finalPosition);
    this.gsplatEntity.setRotation(finalRotation);
  }

  public setSceneTransform(transform: SceneTransform) {
    this.sceneTransform = transform;
    this.updateAlignmentTransform();
  }

  public setCameraMode(mode: CameraMode) {
    this.controller.setMode(mode);
  }

  public setSceneVisible(visible: boolean) {
    if (this.gsplatEntity) {
      this.gsplatEntity.enabled = visible;
    }
  }

  public fitBounds(bounds?: Bounds) {
    this.controller.fitBounds(bounds);
  }

  public loadSplat(url: string, optionsOrBounds?: LoadSplatOptions | Bounds) {
    const options = normalizeLoadOptions(optionsOrBounds);
    const token = ++this.loadToken;
    this.clearSplat();

    if (isRawSplatUrl(url)) {
      void this.loadRawSplat(url, token, options);
      return;
    }

    const asset = new pc.Asset('source', 'gsplat', { url });
    this.gsplatAsset = asset;
    asset.on('progress', (loaded: number, total?: number) => {
      if (this.loadToken !== token) return;
      options.onProgress?.(makeProgress(loaded, total ?? null));
    });
    asset.on('error', (err: unknown) => {
      if (this.loadToken !== token) return;
      options.onError?.(toError(err));
    });
    asset.ready((loaded) => {
      if (this.loadToken !== token) return;
      const resource = loaded.resource as any;
      const summary = sourceSummaryFromResource(resource, options.bounds);
      this.attachGsplatResource(resource, summary.bounds);
      options.onReady?.(summary);
    });

    this.app.assets.add(asset);
    this.app.assets.load(asset);
  }

  public unloadSplat() {
    this.loadToken += 1;
    this.clearSplat();
  }

  private clearSplat() {
    this.loadAbort?.abort();
    this.loadAbort = null;
    if (this.gsplatEntity) {
      this.gsplatEntity.destroy();
      this.gsplatEntity = null;
    }
    if (this.gsplatAsset) {
      this.app.assets.remove(this.gsplatAsset);
      this.gsplatAsset.unload();
      this.gsplatAsset = null;
    }
    if (this.gsplatResource) {
      this.gsplatResource.destroy?.();
      this.gsplatResource = null;
    }
  }

  private async loadRawSplat(url: string, token: number, options: LoadSplatOptions) {
    const controller = new AbortController();
    this.loadAbort = controller;
    try {
      const response = await fetch(url, { signal: controller.signal });
      if (!response.ok) {
        throw new Error(`failed to load ${url}: ${response.status} ${response.statusText}`);
      }
      const bytes = await readResponseBytes(response, (progress) => {
        if (this.loadToken === token) options.onProgress?.(progress);
      });
      if (this.loadToken !== token) return;

      const parsed = parseSplatColumns(bytes);
      const gsplatData = new (pc as any).GSplatData(splatColumnsToPlyElements(parsed));
      const resource = new (pc as any).GSplatResource(this.app.graphicsDevice, gsplatData, {
        prepareCenters: true,
      });
      if (this.loadToken !== token) {
        resource.destroy?.();
        return;
      }

      this.gsplatResource = resource;
      const summary = sourceSummaryFromResource(resource, options.bounds);
      this.attachGsplatResource(resource, summary.bounds);
      options.onReady?.(summary);
    } catch (err) {
      if (this.loadToken !== token) return;
      if (toError(err).name === 'AbortError') return;
      options.onError?.(toError(err));
    } finally {
      if (this.loadToken === token) {
        this.loadAbort = null;
      }
    }
  }

  private attachGsplatResource(resource: any, bounds?: Bounds) {
    this.gsplatEntity = new pc.Entity('source-gsplat');
    this.gsplatEntity.addComponent('gsplat', { unified: true });
    (this.gsplatEntity.gsplat as any).resource = resource;
    this.app.root.addChild(this.gsplatEntity);
    this.bounds = bounds;
    this.updateAlignmentTransform();
    this.controller.fitBounds(bounds);
  }

  public pick(
    screenX: number,
    screenY: number,
  ): Point3 | null {
    const cameraComponent = this.camera.camera;
    if (!cameraComponent) return null;

    const start = cameraComponent.screenToWorld(screenX, screenY, cameraComponent.nearClip);
    const end = cameraComponent.screenToWorld(screenX, screenY, cameraComponent.farClip);

    const worldToLocal = this.gsplatEntity ? this.gsplatEntity.getWorldTransform().clone().invert() : null;
    const localStart = worldToLocal ? worldToLocal.transformPoint(start, new pc.Vec3()) : start;
    const localEnd = worldToLocal ? worldToLocal.transformPoint(end, new pc.Vec3()) : end;
    const localDir = new pc.Vec3().sub2(localEnd, localStart).normalize();

    // Try picking from loaded point cloud centers first
    const resource = (this.gsplatAsset?.resource as any) ?? this.gsplatResource;
    let centers = resource?.centers;
    if (!centers && resource?.gsplatData) {
      centers = resource.gsplatData.getCenters?.();
    }

    if (centers && centers.length > 0) {
      const numSplats = centers.length / 3;
      const threshold = 0.05; // 5cm radius threshold for matching splats
      let bestT = Infinity;
      let bestPoint: Point3 | null = null;
      let fallbackMinDistSq = Infinity;
      let fallbackPoint: Point3 | null = null;

      const startX = localStart.x, startY = localStart.y, startZ = localStart.z;
      const dirX = localDir.x, dirY = localDir.y, dirZ = localDir.z;

      for (let i = 0; i < numSplats; i++) {
        const px = centers[i * 3 + 0];
        const py = centers[i * 3 + 1];
        const pz = centers[i * 3 + 2];

        const vx = px - startX;
        const vy = py - startY;
        const vz = pz - startZ;

        const t = vx * dirX + vy * dirY + vz * dirZ;
        if (t < 0) continue;

        const distSq = (vx * vx + vy * vy + vz * vz) - (t * t);
        if (distSq < threshold * threshold) {
          if (t < bestT) {
            bestT = t;
            bestPoint = [px, py, pz];
          }
        }
        if (distSq < fallbackMinDistSq) {
          fallbackMinDistSq = distSq;
          fallbackPoint = [px, py, pz];
        }
      }

      const resultPoint = bestPoint || fallbackPoint;
      if (resultPoint) {
        return this.applySceneTransformToPoint(resultPoint);
      }
    }

    // Fallback to intersecting with the floor calibration plane
    const planePoint = new pc.Vec3(0, 0, 0);
    const planeNormal = toPcVec(upAxisVector(this.upAxis));
    const previewToEdited = this.alignmentPreviewMatrix().clone().invert();
    const editedStart = previewToEdited.transformPoint(start, new pc.Vec3());
    const editedEnd = previewToEdited.transformPoint(end, new pc.Vec3());
    const intersection = intersectRayPlane(editedStart, editedEnd, planePoint, planeNormal);
    return roundPoint(intersection);
  }

  private alignmentPreviewMatrix() {
    return new pc.Mat4().setTRS(pc.Vec3.ZERO, this.alignmentRotation, pc.Vec3.ONE);
  }

  private applySceneTransformToPoint(point: Point3): Point3 {
    const transformed = sceneTransformRotation(this.sceneTransform)
      .transformVector(toPcVec(point), new pc.Vec3())
      .add(sceneTransformPosition(this.sceneTransform));
    return roundPoint(transformed);
  }

  public getApp(): pc.Application {
    return this.app;
  }

  public destroy() {
    this.unloadSplat();
    this.app.off('update', this.onUpdate);
    this.controller.destroy();
    this.app.destroy();
  }
}

function normalizeLoadOptions(optionsOrBounds?: LoadSplatOptions | Bounds): LoadSplatOptions {
  if (!optionsOrBounds) return {};
  if ('onReady' in optionsOrBounds || 'onProgress' in optionsOrBounds || 'onError' in optionsOrBounds) {
    return optionsOrBounds as LoadSplatOptions;
  }
  return { bounds: optionsOrBounds as Bounds };
}

function isRawSplatUrl(url: string) {
  return url.split(/[?#]/, 1)[0].toLowerCase().endsWith('.splat');
}

function makeProgress(loaded: number, total: number | null): ViewerLoadProgress {
  const validTotal = total && Number.isFinite(total) && total > 0 ? total : null;
  return {
    loaded,
    total: validTotal,
    percent: validTotal ? Math.max(0, Math.min(100, (loaded / validTotal) * 100)) : null,
  };
}

async function readResponseBytes(
  response: Response,
  onProgress: (progress: ViewerLoadProgress) => void,
): Promise<Uint8Array> {
  const total = Number(response.headers.get('content-length')) || null;
  if (!response.body) {
    const buffer = await response.arrayBuffer();
    onProgress(makeProgress(buffer.byteLength, buffer.byteLength));
    return new Uint8Array(buffer);
  }

  const reader = response.body.getReader();
  const chunks: Uint8Array[] = [];
  let loaded = 0;
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    chunks.push(value);
    loaded += value.byteLength;
    onProgress(makeProgress(loaded, total));
  }

  const bytes = new Uint8Array(loaded);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return bytes;
}

function sourceSummaryFromResource(resource: any, fallbackBounds?: Bounds): ViewerSourceSummary {
  const splatCount = Number(resource?.numSplats ?? resource?.gsplatData?.numSplats ?? 0);
  const bounds = boundsFromResource(resource) ?? fallbackBounds ?? {
    min: [0, 0, 0],
    max: [0, 0, 0],
  };
  return { splatCount, bounds };
}

function boundsFromResource(resource: any): Bounds | null {
  const aabb = resource?.aabb;
  if (!aabb?.getMin || !aabb?.getMax) return null;
  const min = aabb.getMin();
  const max = aabb.getMax();
  return {
    min: [min.x, min.y, min.z],
    max: [max.x, max.y, max.z],
  };
}

function sceneTransformPosition(transform: SceneTransform) {
  return new pc.Vec3(transform.position[0], transform.position[1], transform.position[2]);
}

function sceneTransformRotation(transform: SceneTransform) {
  return new pc.Quat().setFromEulerAngles(
    transform.rotationEulerDeg[0],
    transform.rotationEulerDeg[1],
    transform.rotationEulerDeg[2],
  );
}

function toPcVec(point: Point3) {
  return new pc.Vec3(point[0], point[1], point[2]);
}

function toError(value: unknown): Error {
  return value instanceof Error ? value : new Error(String(value));
}

function getRotationBetween(a: pc.Vec3, b: pc.Vec3): pc.Quat {
  const q = new pc.Quat();
  const v0 = a.clone().normalize();
  const v1 = b.clone().normalize();
  const dot = v0.dot(v1);
  if (dot < -0.999999) {
    const axis = new pc.Vec3().cross(new pc.Vec3(1, 0, 0), v0);
    if (axis.lengthSq() < 0.0001) {
      axis.cross(new pc.Vec3(0, 1, 0), v0);
    }
    axis.normalize();
    q.setFromAxisAngle(axis, 180);
  } else if (dot > 0.999999) {
    q.set(0, 0, 0, 1);
  } else {
    const axis = new pc.Vec3().cross(v0, v1);
    q.x = axis.x;
    q.y = axis.y;
    q.z = axis.z;
    q.w = 1 + dot;
    q.normalize();
  }
  return q;
}
