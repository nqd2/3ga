import * as pc from 'playcanvas';
import { CameraController } from './CameraController';
import { calibrationPlane, intersectRayPlane, roundPoint, type Bounds } from './calibration';
import { type Point3 } from './recipe';

export class PlayCanvasViewer {
  private app: pc.Application;
  private camera: pc.Entity;
  private controller: CameraController;
  private canvas: HTMLCanvasElement;
  private gsplatEntity: pc.Entity | null = null;
  private gsplatAsset: pc.Asset | null = null;

  constructor(canvas: HTMLCanvasElement, bounds?: Bounds, sourceUrl?: string | null) {
    this.canvas = canvas;
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
      this.loadSplat(sourceUrl, bounds);
    }

    this.app.start();
  }

  private onUpdate = (dt: number) => {
    this.controller.update(dt);
  };

  public fitBounds(bounds?: Bounds) {
    this.controller.fitBounds(bounds);
  }

  public loadSplat(url: string, bounds?: Bounds) {
    this.unloadSplat();

    this.gsplatAsset = new pc.Asset('source', 'gsplat', { url });
    this.gsplatAsset.ready((loaded) => {
      this.gsplatEntity = new pc.Entity('source-gsplat');
      this.gsplatEntity.addComponent('gsplat', { asset: loaded, unified: true });
      this.app.root.addChild(this.gsplatEntity);
      this.controller.fitBounds(bounds);
    });

    this.app.assets.add(this.gsplatAsset);
    this.app.assets.load(this.gsplatAsset);
  }

  public unloadSplat() {
    if (this.gsplatEntity) {
      this.gsplatEntity.destroy();
      this.gsplatEntity = null;
    }
    if (this.gsplatAsset) {
      this.app.assets.remove(this.gsplatAsset);
      this.gsplatAsset.unload();
      this.gsplatAsset = null;
    }
  }

  public pick(
    screenX: number,
    screenY: number,
    floorPoints: [Point3, Point3, Point3]
  ): Point3 | null {
    const cameraComponent = this.camera.camera;
    if (!cameraComponent) return null;

    const start = cameraComponent.screenToWorld(screenX, screenY, cameraComponent.nearClip);
    const end = cameraComponent.screenToWorld(screenX, screenY, cameraComponent.farClip);

    const plane = calibrationPlane(floorPoints);
    const intersection = intersectRayPlane(start, end, plane.point, plane.normal);
    return roundPoint(intersection);
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
