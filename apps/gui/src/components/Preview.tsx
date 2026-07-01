import React, { useEffect, useRef } from 'react';
import { EmptyState } from '@astryxdesign/core/EmptyState';
import { Banner } from '@astryxdesign/core/Banner';
import { Tooltip } from '@astryxdesign/core/Tooltip';
import { Text } from '@astryxdesign/core/Text';
import { AlertTriangle } from 'lucide-react';
import { PlayCanvasViewer, type ViewerLoadProgress, type ViewerSourceSummary } from '../classes/PlayCanvasViewer';
import { type CameraMode } from '../classes/CameraController';
import { type SceneTransform } from '../domains/editor/sceneTransform';
import { type Bounds } from '../domains/calibration';
import { type Point3, type PickMode, type UpAxis } from '../domains/calibration';

export function getPickModeLabel(mode: PickMode): string {
  switch (mode) {
    case 'scale0': return 'Scale Endpoint 1';
    case 'scale1': return 'Scale Endpoint 2';
    default: return '';
  }
}

export type SourceMetadata = {
  path: string;
  bytes: number;
  format: string;
  splatCount: number;
  bounds?: Bounds;
  previewPath?: string;
};

export function Preview({
  sourceUrl,
  sourceMetadata,
  bounds,
  scalePoints,
  onPick,
  pickMode,
  onSourceProgress,
  onSourceReady,
  onSourceError,
  upAxis,
  sceneTransform,
  cameraMode,
  sceneVisible = true,
}: {
  sourceUrl: string | null;
  sourceMetadata: SourceMetadata | null;
  bounds?: Bounds;
  scalePoints: [Point3, Point3];
  onPick: (point: Point3) => void;
  pickMode: PickMode;
  onSourceProgress: (progress: ViewerLoadProgress) => void;
  onSourceReady: (summary: ViewerSourceSummary) => void;
  onSourceError: (error: Error) => void;
  upAxis: UpAxis;
  sceneTransform: SceneTransform;
  cameraMode: CameraMode;
  sceneVisible?: boolean;
}) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const viewerRef = useRef<PlayCanvasViewer | null>(null);
  const startPos = useRef({ x: 0, y: 0 });
  const hasViewportSource = Boolean(sourceMetadata || sourceUrl);

  // Sync calibration updates to PlayCanvas overlay
  useEffect(() => {
    if (viewerRef.current) {
      viewerRef.current.updateCalibration(scalePoints, bounds, upAxis);
    }
  }, [scalePoints, bounds, upAxis]);

  useEffect(() => {
    viewerRef.current?.setSceneTransform(sceneTransform);
  }, [sceneTransform]);

  useEffect(() => {
    viewerRef.current?.setCameraMode(cameraMode);
  }, [cameraMode]);

  useEffect(() => {
    viewerRef.current?.setSceneVisible(sceneVisible);
  }, [sceneVisible]);

  // Load model when sourceUrl changes
  useEffect(() => {
    if (!viewerRef.current) return;
    if (sourceUrl) {
      viewerRef.current.loadSplat(sourceUrl, {
        onProgress: onSourceProgress,
        onReady: onSourceReady,
        onError: onSourceError,
      });
    } else {
      viewerRef.current.unloadSplat();
    }
  }, [onSourceError, onSourceProgress, onSourceReady, sourceUrl]);

  // Initialize viewer after the viewport canvas exists.
  useEffect(() => {
    if (!hasViewportSource || viewerRef.current) return;
    const canvas = canvasRef.current;
    if (!canvas) return;

    const viewer = new PlayCanvasViewer(canvas, bounds);
    viewer.setSceneTransform(sceneTransform);
    viewer.setCameraMode(cameraMode);
    viewer.updateCalibration(scalePoints, bounds, upAxis);
    viewer.setSceneVisible(sceneVisible);
    viewerRef.current = viewer;
    if (sourceUrl) {
      viewer.loadSplat(sourceUrl, {
        onProgress: onSourceProgress,
        onReady: onSourceReady,
        onError: onSourceError,
      });
    }

    return () => {
      viewer.destroy();
      viewerRef.current = null;
    };
  }, [hasViewportSource]);

  function handlePointerDown(event: React.PointerEvent<HTMLCanvasElement>) {
    if (event.button !== 0 || event.shiftKey) return;
    startPos.current = { x: event.clientX, y: event.clientY };
  }

  function handlePointerUp(event: React.PointerEvent<HTMLCanvasElement>) {
    if (event.button !== 0 || event.shiftKey) return;
    const dx = event.clientX - startPos.current.x;
    const dy = event.clientY - startPos.current.y;
    // Trigger pick only if it's a static click (under 5px movement threshold)
    if (Math.sqrt(dx * dx + dy * dy) < 5) {
      const rect = event.currentTarget.getBoundingClientRect();
      const x = event.clientX - rect.left;
      const y = event.clientY - rect.top;
      if (viewerRef.current) {
        const point = viewerRef.current.pick(x, y);
        if (point) onPick(point);
      }
    }
  }

  if (!hasViewportSource) {
    return (
      <div className="viewport-empty-state">
        <EmptyState
          title="No Source Loaded"
          description="Load a 3D Gaussian Splat (.ply or .splat) file to begin calibration."
          icon={<AlertTriangle size={36} color="var(--color-text-secondary)" />}
        />
      </div>
    );
  }

  return (
    <div className="viewport">
      <canvas className="preview" ref={canvasRef} onPointerDown={handlePointerDown} onPointerUp={handlePointerUp} />

      <div className="viewport-legend">
        <div className="legend-item">
          <Tooltip content="Scale endpoints for physical distance reference">
            <span className="legend-color-dot" data-marker="scale" />
          </Tooltip>
          <Text type="label" size="sm" color="secondary" as="span">Scale Endpoint</Text>
        </div>
        <div className="legend-item">
          <Tooltip content="Calculated coordinate origin and up axis">
            <span className="legend-color-dot" data-marker="origin" />
          </Tooltip>
          <Text type="label" size="sm" color="secondary" as="span">Origin</Text>
        </div>
      </div>

      <div className="viewport-overlay-hint">
        {cameraMode === 'fly'
          ? 'Left-click/Drag: Look • WASD: Move • Q/E: Vertical • Shift/Alt: Speed • Scroll: Forward'
          : 'Left-click/Drag: Orbit • Shift+Left-click/Drag: Pan • Scroll: Zoom • Left-click: Place marker'}
      </div>
    </div>
  );
}
