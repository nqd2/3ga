import * as pc from 'playcanvas';
import { type Bounds } from './calibration';

export class CameraController {
  private cameraEntity: pc.Entity;
  private canvas: HTMLCanvasElement;

  // Spherical Coordinates (current)
  private currentTheta = Math.PI / 4;
  private currentPhi = Math.PI / 3;
  private currentRadius = 5.0;
  private currentLookAt = new pc.Vec3(0, 0, 0);

  // Spherical Coordinates (target)
  private targetTheta = Math.PI / 4;
  private targetPhi = Math.PI / 3;
  private targetRadius = 5.0;
  private targetLookAt = new pc.Vec3(0, 0, 0);

  // Dynamic limits based on bounding box
  private minRadius = 0.1;
  private maxRadius = 100.0;

  // Interactive tracking
  private activePointers = new Map<number, { clientX: number; clientY: number }>();
  private isOrbiting = false;
  private isPanning = false;
  private lastPinchDistance = 0;
  private lastPinchCenter = new pc.Vec2(0, 0);

  constructor(cameraEntity: pc.Entity, canvas: HTMLCanvasElement) {
    this.cameraEntity = cameraEntity;
    this.canvas = canvas;
    this.setupListeners();
  }

  private setupListeners() {
    this.canvas.addEventListener('contextmenu', this.handleContextMenu);
    this.canvas.addEventListener('pointerdown', this.handlePointerDown);
    this.canvas.addEventListener('pointermove', this.handlePointerMove);
    this.canvas.addEventListener('pointerup', this.handlePointerUp);
    this.canvas.addEventListener('pointercancel', this.handlePointerUp);
    this.canvas.addEventListener('wheel', this.handleWheel, { passive: false });
  }

  public destroy() {
    this.canvas.removeEventListener('contextmenu', this.handleContextMenu);
    this.canvas.removeEventListener('pointerdown', this.handlePointerDown);
    this.canvas.removeEventListener('pointermove', this.handlePointerMove);
    this.canvas.removeEventListener('pointerup', this.handlePointerUp);
    this.canvas.removeEventListener('pointercancel', this.handlePointerUp);
    this.canvas.removeEventListener('wheel', this.handleWheel);
  }

  private handleContextMenu = (e: MouseEvent) => {
    e.preventDefault();
  };

  private handlePointerDown = (e: PointerEvent) => {
    this.canvas.setPointerCapture(e.pointerId);
    this.activePointers.set(e.pointerId, { clientX: e.clientX, clientY: e.clientY });

    if (this.activePointers.size === 1) {
      if (e.button === 2 || e.shiftKey) {
        this.isPanning = true;
        this.isOrbiting = false;
      } else {
        this.isOrbiting = true;
        this.isPanning = false;
      }
    } else if (this.activePointers.size === 2) {
      this.isOrbiting = false;
      this.isPanning = false;
      const pts = Array.from(this.activePointers.values());
      this.lastPinchDistance = this.getPinchDistance(pts[0], pts[1]);
      this.lastPinchCenter = this.getPinchCenter(pts[0], pts[1]);
    }
  };

  private handlePointerMove = (e: PointerEvent) => {
    if (!this.activePointers.has(e.pointerId)) return;
    
    const prev = this.activePointers.get(e.pointerId)!;
    const dx = e.clientX - prev.clientX;
    const dy = e.clientY - prev.clientY;
    
    // Update pointer position
    this.activePointers.set(e.pointerId, { clientX: e.clientX, clientY: e.clientY });

    if (this.activePointers.size === 1) {
      if (this.isOrbiting) {
        const orbitScale = 0.005;
        this.targetTheta -= dx * orbitScale;
        this.targetPhi = Math.max(
          1 * Math.PI / 180, // Clamp polar angle min 1 degree
          Math.min(179 * Math.PI / 180, this.targetPhi + dy * orbitScale) // Clamp polar angle max 179 degree
        );
      } else if (this.isPanning) {
        this.pan(dx, dy);
      }
    } else if (this.activePointers.size === 2) {
      const pts = Array.from(this.activePointers.values());
      const dist = this.getPinchDistance(pts[0], pts[1]);
      const center = this.getPinchCenter(pts[0], pts[1]);

      // Pinch zoom
      if (this.lastPinchDistance > 0 && dist > 0) {
        const ratio = this.lastPinchDistance / dist;
        this.targetRadius = Math.max(
          this.minRadius,
          Math.min(this.maxRadius, this.targetRadius * ratio)
        );
      }

      // Midpoint pan
      const panDx = center.x - this.lastPinchCenter.x;
      const panDy = center.y - this.lastPinchCenter.y;
      this.pan(panDx, panDy);

      this.lastPinchDistance = dist;
      this.lastPinchCenter = center;
    }
  };

  private handlePointerUp = (e: PointerEvent) => {
    try {
      this.canvas.releasePointerCapture(e.pointerId);
    } catch {}
    this.activePointers.delete(e.pointerId);

    if (this.activePointers.size === 0) {
      this.isOrbiting = false;
      this.isPanning = false;
    } else if (this.activePointers.size === 1) {
      // Transition back to single touch tracking
      this.isOrbiting = true;
      this.isPanning = false;
    }
  };

  private handleWheel = (e: WheelEvent) => {
    e.preventDefault();
    const zoomIntensity = 0.05;
    const factor = e.deltaY > 0 ? 1.1 : 0.9;
    this.targetRadius = Math.max(
      this.minRadius,
      Math.min(this.maxRadius, this.targetRadius * (1.0 + (factor - 1.0) * zoomIntensity * 10))
    );
  };

  private pan(dx: number, dy: number) {
    const fov = this.cameraEntity.camera?.fov || 45;
    const fovRad = (fov * Math.PI) / 180;
    const factor = (2.0 * this.currentRadius * Math.tan(fovRad / 2.0)) / this.canvas.clientHeight;

    const transform = this.cameraEntity.getWorldTransform();
    const right = transform.getX(new pc.Vec3());
    const up = transform.getY(new pc.Vec3());

    const panOffset = new pc.Vec3()
      .addScaled(right, -dx * factor)
      .addScaled(up, dy * factor);

    this.targetLookAt.add(panOffset);
    this.currentLookAt.add(panOffset);
  }

  private getPinchDistance(p1: { clientX: number; clientY: number }, p2: { clientX: number; clientY: number }): number {
    const dx = p1.clientX - p2.clientX;
    const dy = p1.clientY - p2.clientY;
    return Math.sqrt(dx * dx + dy * dy);
  }

  private getPinchCenter(p1: { clientX: number; clientY: number }, p2: { clientX: number; clientY: number }): pc.Vec2 {
    return new pc.Vec2((p1.clientX + p2.clientX) * 0.5, (p1.clientY + p2.clientY) * 0.5);
  }

  public fitBounds(bounds?: Bounds) {
    const min = bounds?.min ?? { x: -2, y: -1, z: -2 };
    const max = bounds?.max ?? { x: 2, y: 2, z: 2 };
    
    const center = new pc.Vec3(
      (min.x + max.x) * 0.5,
      (min.y + max.y) * 0.5,
      (min.z + max.z) * 0.5
    );
    const size = new pc.Vec3(max.x - min.x, max.y - min.y, max.z - min.z);
    const radius = Math.max(size.length() * 0.75, 3.0);

    // Set dynamic radius limits (10% to 1000% of bounding radius)
    this.minRadius = radius * 0.1;
    this.maxRadius = radius * 10.0;

    // Trigger smooth lerp to new target
    this.targetLookAt.copy(center);
    this.targetRadius = radius;
    this.targetTheta = Math.PI / 4;
    this.targetPhi = Math.PI / 3;
  }

  public update(dt: number) {
    // Damping factor (independent of framerate)
    const damping = Math.min(15.0 * dt, 1.0);

    this.currentTheta += (this.targetTheta - this.currentTheta) * damping;
    this.currentPhi += (this.targetPhi - this.currentPhi) * damping;
    this.currentRadius += (this.targetRadius - this.currentRadius) * damping;
    this.currentLookAt.lerp(this.currentLookAt, this.targetLookAt, damping);

    // Convert spherical coordinates back to Cartesian positions (Y-up convention)
    const x = this.currentLookAt.x + this.currentRadius * Math.sin(this.currentPhi) * Math.sin(this.currentTheta);
    const y = this.currentLookAt.y + this.currentRadius * Math.cos(this.currentPhi);
    const z = this.currentLookAt.z + this.currentRadius * Math.sin(this.currentPhi) * Math.cos(this.currentTheta);

    this.cameraEntity.setPosition(x, y, z);
    this.cameraEntity.lookAt(this.currentLookAt);
  }
}
