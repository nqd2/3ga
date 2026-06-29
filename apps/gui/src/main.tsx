import React, { useEffect, useMemo, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Theme, TextInput, NumberInput, Selector } from '@astryxdesign/core';
import { neutralTheme } from '@astryxdesign/theme-neutral';
import '@astryxdesign/theme-neutral/theme.css';

import {
  defaultConfig,
  defaultFloorFitPoints,
  defaultFloorPoints,
  defaultScalePoints,
  makeAlignmentRecipe,
  type FloorMode,
  type Point3,
  type UpAxis,
} from './recipe';
import { PlayCanvasViewer } from './PlayCanvasViewer';
import { previewFloorPoints, type Bounds } from './calibration';
import './styles.css';

type Manifest = {
  version: number;
  source: { format: string; splatCount: number; keptCount: number };
  artifacts: { manifest: string; collisionMeshJson: string };
  metrics: Record<string, number | string | null>;
};

type SourceMetadata = {
  path: string;
  bytes: number;
  format: string;
  splatCount: number;
  bounds?: Bounds;
};

type ProcessRequest = {
  inputPath: string;
  outDir: string;
  configJson: string;
  recipeJson: string;
};

type JobProgress = { stage: string };

type PickMode = 'floor0' | 'floor1' | 'floor2' | 'floorFit' | 'scale0' | 'scale1';

function Preview({
  sourceUrl,
  bounds,
  floorPoints,
  scalePoints,
  onPick,
}: {
  sourceUrl: string | null;
  bounds?: Bounds;
  floorPoints: [Point3, Point3, Point3];
  scalePoints: [Point3, Point3];
  onPick: (point: Point3) => void;
}) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const viewerRef = useRef<PlayCanvasViewer | null>(null);
  const startPos = useRef({ x: 0, y: 0 });

  // Sync calibration updates to PlayCanvas overlay
  useEffect(() => {
    if (viewerRef.current) {
      viewerRef.current.updateCalibration(floorPoints, scalePoints, bounds);
    }
  }, [floorPoints, scalePoints, bounds]);

  // Load model when sourceUrl changes
  useEffect(() => {
    if (viewerRef.current && sourceUrl) {
      viewerRef.current.loadSplat(sourceUrl, bounds);
    }
  }, [sourceUrl, bounds]);

  // Initialize viewer
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const viewer = new PlayCanvasViewer(canvas, bounds, sourceUrl);
    viewer.updateCalibration(floorPoints, scalePoints, bounds);
    viewerRef.current = viewer;

    return () => {
      viewer.destroy();
      viewerRef.current = null;
    };
  }, []);

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
        const point = viewerRef.current.pick(x, y, floorPoints);
        if (point) onPick(point);
      }
    }
  }

  return (
    <div className="viewport">
      <canvas className="preview" ref={canvasRef} onPointerDown={handlePointerDown} onPointerUp={handlePointerUp} />
      <div className="viewport-overlay-hint">
        Left-click/Drag: Orbit • Shift+Left-click/Drag: Pan • Scroll: Zoom • Left-click: Place marker
      </div>
    </div>
  );
}

function PointEditor({
  title,
  points,
  onChange,
}: {
  title: string;
  points: readonly Point3[];
  onChange: (index: number, point: Point3) => void;
}) {
  return (
    <div className="point-editor">
      <h2>{title}</h2>
      {points.map((point, pointIndex) => (
        <div key={`${title}-${pointIndex}`}>
          <div className="point-row-label">{title.replace('endpoints', '')} {pointIndex + 1}</div>
          <div className="point-row">
            {point.map((value, axis) => (
              <NumberInput
                key={axis}
                label={`${title} ${pointIndex + 1} ${['x', 'y', 'z'][axis]}`}
                isLabelHidden
                step={0.001}
                value={value}
                onChange={(val) => {
                  const next: Point3 = [...point] as Point3;
                  next[axis] = val;
                  onChange(pointIndex, next);
                }}
              />
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}

function sourcePathToUrl(path: string) {
  try {
    return convertFileSrc(path);
  } catch {
    return null;
  }
}

function App() {
  const [inputPath, setInputPath] = useState('');
  const [outDir, setOutDir] = useState('target/gui-output');
  const [distance, setDistance] = useState(2);
  const [floorMode, setFloorMode] = useState<FloorMode>('three-point');
  const [floorPoints, setFloorPoints] =
    useState<[Point3, Point3, Point3]>(defaultFloorPoints);
  const [floorFitPoints, setFloorFitPoints] = useState<Point3[]>(defaultFloorFitPoints);
  const [scalePoints, setScalePoints] =
    useState<[Point3, Point3]>(defaultScalePoints);
  const [upAxis, setUpAxis] = useState<UpAxis>('y');
  const [pickMode, setPickMode] = useState<PickMode>('floor0');
  const [status, setStatus] = useState('idle');
  const [isProcessing, setIsProcessing] = useState(false);
  const [manifest, setManifest] = useState<Manifest | null>(null);
  const [sourceMetadata, setSourceMetadata] = useState<SourceMetadata | null>(null);
  const [sourceUrl, setSourceUrl] = useState<string | null>(null);

  const recipe = useMemo(
    () => makeAlignmentRecipe(distance, floorPoints, scalePoints, upAxis, floorMode, floorFitPoints),
    [distance, floorFitPoints, floorMode, floorPoints, scalePoints, upAxis],
  );
  const activeFloorPoints = useMemo(
    () => previewFloorPoints(floorMode, floorPoints, floorFitPoints),
    [floorFitPoints, floorMode, floorPoints],
  );

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen<JobProgress>('job-progress', (event) => {
      setStatus(`processing: ${event.payload.stage}`);
    })
      .then((handler) => {
        unlisten = handler;
      })
      .catch(() => {});
    return () => unlisten?.();
  }, []);

  function updateFloorPoint(index: 0 | 1 | 2, point: Point3) {
    setFloorPoints((current) => {
      const next: [Point3, Point3, Point3] = [[...current[0]], [...current[1]], [...current[2]]];
      next[index] = point;
      return next;
    });
  }

  function updateScalePoint(index: 0 | 1, point: Point3) {
    setScalePoints((current) => {
      const next: [Point3, Point3] = [[...current[0]], [...current[1]]];
      next[index] = point;
      return next;
    });
  }

  function updateFloorFitPoint(index: number, point: Point3) {
    setFloorFitPoints((current) => current.map((value, pointIndex) => (
      pointIndex === index ? point : value
    )));
  }

  function handlePick(point: Point3) {
    if (pickMode === 'floor0') updateFloorPoint(0, point);
    if (pickMode === 'floor1') updateFloorPoint(1, point);
    if (pickMode === 'floor2') updateFloorPoint(2, point);
    if (pickMode === 'floorFit') setFloorFitPoints((current) => [...current, point]);
    if (pickMode === 'scale0') updateScalePoint(0, point);
    if (pickMode === 'scale1') updateScalePoint(1, point);
  }

  async function loadSource() {
    setStatus('loading source');
    try {
      const metadata = await invoke<SourceMetadata>('load_source', { path: inputPath });
      setSourceMetadata(metadata);
      setSourceUrl(sourcePathToUrl(metadata.path));
      setStatus('source loaded');
    } catch (error) {
      setStatus(String(error));
    }
  }

  async function runProcess() {
    if (isProcessing) return;
    setStatus('processing');
    setIsProcessing(true);
    setManifest(null);
    try {
      const request: ProcessRequest = {
        inputPath,
        outDir,
        configJson: JSON.stringify(defaultConfig),
        recipeJson: JSON.stringify(recipe),
      };
      const result = await invoke<Manifest>('process_job', { request });
      setManifest(result);
      setStatus('done');
    } catch (error) {
      setStatus(String(error));
    } finally {
      setIsProcessing(false);
    }
  }

  async function cancelJob() {
    await invoke<boolean>('cancel_job');
    setStatus('cancel requested');
  }

  async function saveBundle() {
    if (!manifest) return;
    const destinationPath = `${outDir}/webar-copy.zip`;
    try {
      await invoke<string>('save_bundle', { request: { outDir, destinationPath } });
      setStatus(`saved ${destinationPath}`);
    } catch (error) {
      setStatus(String(error));
    }
  }

  async function previewWebAr() {
    if (!manifest) return;
    const indexPath = `${outDir}/index.html`;
    try {
      await invoke('open_webar_viewer', { path: indexPath });
      setStatus('preview opened');
    } catch (error) {
      setStatus(String(error));
    }
  }

  return (
    <Theme theme={neutralTheme} mode="dark">
      <main className="app-shell">
        <header className="toolbar">
          <div>
            <p className="eyebrow">3DGS to AR geometry</p>
            <h1>augmented-gaussian</h1>
          </div>
          <div className="toolbar-actions">
            <button className="btn-secondary" disabled={!inputPath || isProcessing} onClick={loadSource}>
              Load Source
            </button>
            <button className="btn-primary" disabled={!inputPath || isProcessing} onClick={runProcess}>
              Bake Geometry
            </button>
            <button className="btn-secondary" disabled={!isProcessing} onClick={cancelJob}>
              Cancel Job
            </button>
          </div>
        </header>

        <section className="workspace">
          <aside className="panel">
            <h2 className="panel-header">Configuration</h2>
            <TextInput
              label="Source PLY/SPLAT Path"
              value={inputPath}
              onChange={(val) => setInputPath(val)}
              placeholder="e.g. models/room.ply"
            />
            <TextInput
              label="Output Directory"
              value={outDir}
              onChange={(val) => setOutDir(val)}
            />
            <NumberInput
              label="Scale Calibration Distance (m)"
              min={0.001}
              step={0.1}
              value={distance}
              onChange={(val) => setDistance(val)}
            />
            <Selector
              label="Floor Mode"
              options={[
                { value: 'three-point', label: '3 points' },
                { value: 'fit', label: 'Fit selected points' },
              ]}
              value={floorMode}
              onChange={(val) => setFloorMode(val as FloorMode)}
            />
            <Selector
              label="Pick Target"
              options={[
                { value: 'floor0', label: 'Floor Point 1' },
                { value: 'floor1', label: 'Floor Point 2' },
                { value: 'floor2', label: 'Floor Point 3' },
                { value: 'floorFit', label: 'Floor Fit Point' },
                { value: 'scale0', label: 'Scale Endpoint 1' },
                { value: 'scale1', label: 'Scale Endpoint 2' },
              ]}
              value={pickMode}
              onChange={(val) => setPickMode(val as PickMode)}
            />
            <Selector
              label="Up Axis"
              options={[
                { value: 'y', label: 'Y up' },
                { value: 'z', label: 'Z up' },
                { value: 'x', label: 'X up' },
                { value: 'neg-y', label: '-Y up' },
                { value: 'neg-z', label: '-Z up' },
                { value: 'neg-x', label: '-X up' },
              ]}
              value={upAxis}
              onChange={(val) => setUpAxis(val as UpAxis)}
            />
          </aside>

          <section className="viewport-container">
            <Preview
              sourceUrl={sourceUrl}
              bounds={sourceMetadata?.bounds}
              floorPoints={activeFloorPoints}
              scalePoints={scalePoints}
              onPick={handlePick}
            />
          </section>

          <aside className="panel">
            <h2 className="panel-header">Pipeline Status</h2>
            <div className={`status ${status === 'idle' ? 'idle' : status.startsWith('error') ? 'error' : ''}`}>
              {status}
            </div>

            {sourceMetadata && (
              <>
                <h2 className="panel-header" style={{ marginTop: '16px' }}>Metadata</h2>
                <dl className="metadata-list">
                  <div className="metadata-row">
                    <dt>Format</dt>
                    <dd>{sourceMetadata.format}</dd>
                  </div>
                  <div className="metadata-row">
                    <dt>Splat Count</dt>
                    <dd>{sourceMetadata.splatCount.toLocaleString()}</dd>
                  </div>
                </dl>
              </>
            )}

            {manifest && (
              <>
                <h2 className="panel-header" style={{ marginTop: '16px' }}>Manifest Details</h2>
                <dl className="metadata-list">
                  <div className="metadata-row">
                    <dt>Output Format</dt>
                    <dd>{manifest.source.format}</dd>
                  </div>
                  <div className="metadata-row">
                    <dt>Baker Splats</dt>
                    <dd>{manifest.source.splatCount.toLocaleString()}</dd>
                  </div>
                  <div className="metadata-row">
                    <dt>Triangles</dt>
                    <dd>{manifest.metrics.collisionTrianglesAfterMerge || 0}</dd>
                  </div>
                </dl>
                <button
                  style={{ width: '100%', marginTop: '16px' }}
                  className="btn-primary"
                  disabled={!manifest || isProcessing}
                  onClick={saveBundle}
                >
                  Save WebAR Bundle
                </button>
                <button
                  style={{ width: '100%', marginTop: '8px' }}
                  className="btn-secondary"
                  disabled={!manifest || isProcessing}
                  onClick={previewWebAr}
                >
                  Preview WebAR in App
                </button>
              </>
            )}
          </aside>
        </section>
      </main>
    </Theme>
  );
}

const container = document.getElementById('root');
if (container) {
  createRoot(container).render(<App />);
}
