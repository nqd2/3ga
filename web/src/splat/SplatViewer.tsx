import { Upload } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import type { EditRecipe } from '../edit/EditStore';
import { parsePreviewFile, type PreviewBounds, type PreviewData } from './previewParser';
import { open } from '@tauri-apps/plugin-dialog';
import { convertFileSrc } from '@tauri-apps/api/core';

type PreviewState =
  | { kind: 'empty' }
  | { kind: 'loading'; fileName: string }
  | { kind: 'ready'; data: PreviewData }
  | { kind: 'error'; fileName: string; message: string };

export function SplatViewer({
  inputPath,
  recipe,
  onInputPathChange,
}: {
  inputPath: string;
  recipe: EditRecipe;
  onInputPathChange?: (path: string) => void;
}) {
  const [preview, setPreview] = useState<PreviewState>({ kind: 'empty' });

  const isTauri = typeof window !== 'undefined' && !!(window as any).__TAURI_INTERNALS__;

  const loadPreview = async (file: File | undefined, absolutePath?: string) => {
    if (!file) {
      return;
    }
    setPreview({ kind: 'loading', fileName: file.name });
    try {
      const data = await parsePreviewFile(file, { maxPoints: 60000 });
      setPreview({ kind: 'ready', data });

      if (onInputPathChange) {
        if (isTauri && absolutePath) {
          onInputPathChange(absolutePath);
        } else {
          const formData = new FormData();
          formData.append('file', file);
          const response = await fetch('/api/upload', {
            method: 'POST',
            body: formData,
          });
          if (!response.ok) {
            throw new Error(`Upload failed: ${response.statusText}`);
          }
          const res = await response.json();
          onInputPathChange(res.path);
        }
      }
    } catch (error) {
      setPreview({ kind: 'error', fileName: file.name, message: error instanceof Error ? error.message : 'Preview parse failed' });
    }
  };

  const handleTauriPick = async (e: React.MouseEvent) => {
    e.preventDefault();
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        filters: [{ name: 'Gaussian Splats', extensions: ['ply', 'splat', 'sog'] }]
      });
      if (selected && typeof selected === 'string') {
        const assetUrl = convertFileSrc(selected);
        const resp = await fetch(assetUrl);
        const blob = await resp.blob();
        const fileName = selected.split(/[/\\]/).pop() || 'file.ply';
        const fileObj = new File([blob], fileName);
        await loadPreview(fileObj, selected);
      }
    } catch (err) {
      console.error('Tauri file picker error:', err);
    }
  };

  return (
    <section className="viewer" aria-label="Splat preview">
      <div className="viewer__topline">
        <span>Source preview</span>
        <strong>{preview.kind === 'ready' ? preview.data.format.toUpperCase() : preview.kind === 'loading' ? 'Parsing' : 'No local file'}</strong>
      </div>

      <div className="viewer__controls">
        <label className="file-picker" onClick={isTauri ? handleTauriPick : undefined}>
          <Upload size={16} />
          <span>Choose local .ply, .splat, or .sog</span>
          {!isTauri && (
            <input
              type="file"
              accept=".ply,.splat,.sog"
              onChange={(event) => {
                void loadPreview(event.currentTarget.files?.[0]);
              }}
            />
          )}
        </label>
        <div className="viewer__path">
          <span>Server process path</span>
          <code>{inputPath || 'none'}</code>
        </div>
      </div>

      {preview.kind === 'ready' ? (
        <div className="viewer__body">
          <GaussianCanvas data={preview.data} />
          <PreviewStats data={preview.data} editCount={recipe.operations.length} />
        </div>
      ) : (
        <div className="viewer__empty">
          <div>
            <span>Edit operations</span>
            <strong>{recipe.operations.length}</strong>
          </div>
          {preview.kind === 'loading' ? (
            <p>Parsing {preview.fileName} from local browser bytes...</p>
          ) : preview.kind === 'error' ? (
            <p className="viewer__error">{preview.fileName}: {preview.message}</p>
          ) : (
            <p>Select a local source file to inspect real preview data. Processing still uses the server path and recipe JSON on the right.</p>
          )}
        </div>
      )}
    </section>
  );
}

function PreviewStats({ data, editCount }: { data: PreviewData; editCount: number }) {
  return (
    <aside className="preview-stats" aria-label="Preview stats">
      <div>
        <span>File</span>
        <strong>{data.fileName}</strong>
      </div>
      <div>
        <span>Splats</span>
        <strong>{data.splatCount.toLocaleString()}</strong>
      </div>
      <div>
        <span>Displayed</span>
        <strong>{data.displayedCount.toLocaleString()}</strong>
      </div>
      <div>
        <span>Bounds</span>
        <code>{formatBounds(data.bounds)}</code>
      </div>
      <div>
        <span>Edit operations</span>
        <strong>{editCount}</strong>
      </div>
      <p>Preview uses this local file only. Process uses the server path field.</p>
      {data.warnings.map((warning) => (
        <p key={warning}>{warning}</p>
      ))}
    </aside>
  );
}

function GaussianCanvas({ data }: { data: PreviewData }) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const [rotation, setRotation] = useState({ yaw: 0.5, pitch: -0.3 });
  const [isDragging, setIsDragging] = useState(false);
  const dragStartRef = useRef<{ x: number; y: number; yaw: number; pitch: number } | null>(null);

  const handlePointerDown = (e: React.PointerEvent<HTMLCanvasElement>) => {
    e.currentTarget.setPointerCapture(e.pointerId);
    setIsDragging(true);
    dragStartRef.current = {
      x: e.clientX,
      y: e.clientY,
      yaw: rotation.yaw,
      pitch: rotation.pitch,
    };
  };

  const handlePointerMove = (e: React.PointerEvent<HTMLCanvasElement>) => {
    if (!dragStartRef.current) return;
    const dx = e.clientX - dragStartRef.current.x;
    const dy = e.clientY - dragStartRef.current.y;
    setRotation({
      yaw: dragStartRef.current.yaw - dx * 0.007,
      pitch: Math.max(-Math.PI / 2 + 0.01, Math.min(Math.PI / 2 - 0.01, dragStartRef.current.pitch + dy * 0.007)),
    });
  };

  const handlePointerUp = (e: React.PointerEvent<HTMLCanvasElement>) => {
    e.currentTarget.releasePointerCapture(e.pointerId);
    setIsDragging(false);
    dragStartRef.current = null;
  };

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) {
      return;
    }
    const rect = canvas.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    const width = Math.max(1, Math.floor(rect.width * dpr));
    const height = Math.max(1, Math.floor(rect.height * dpr));
    canvas.width = width;
    canvas.height = height;
    const ctx = canvas.getContext('2d');
    if (!ctx) {
      return;
    }

    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, rect.width, rect.height);
    ctx.fillStyle = '#071014';
    ctx.fillRect(0, 0, rect.width, rect.height);

    const centerX = (data.bounds.min[0] + data.bounds.max[0]) * 0.5;
    const centerY = (data.bounds.min[1] + data.bounds.max[1]) * 0.5;
    const centerZ = (data.bounds.min[2] + data.bounds.max[2]) * 0.5;

    const spanX = Math.max(1e-5, data.bounds.max[0] - data.bounds.min[0]);
    const spanY = Math.max(1e-5, data.bounds.max[1] - data.bounds.min[1]);
    const spanZ = Math.max(1e-5, data.bounds.max[2] - data.bounds.min[2]);

    const maxDim = Math.max(spanX, spanY, spanZ);
    const pad = 24;
    const scale = Math.min((rect.width - pad * 2) / maxDim, (rect.height - pad * 2) / maxDim);

    const cosY = Math.cos(rotation.yaw);
    const sinY = Math.sin(rotation.yaw);
    const cosP = Math.cos(rotation.pitch);
    const sinP = Math.sin(rotation.pitch);

    // Transform and project points using rotation matrix and orthographic projection
    const projected = data.points.map((point) => {
      const dx = point.x - centerX;
      const dy = point.y - centerY;
      const dz = point.z - centerZ;

      // Rotate around Y axis (yaw)
      const rx1 = dx * cosY - dz * sinY;
      const rz1 = dx * sinY + dz * cosY;
      const ry1 = dy;

      // Rotate around X axis (pitch)
      const rx2 = rx1;
      const ry2 = ry1 * cosP - rz1 * sinP;
      const rz2 = ry1 * sinP + rz1 * cosP;

      return {
        x: rect.width * 0.5 + rx2 * scale,
        y: rect.height * 0.5 - ry2 * scale,
        depth: rz2,
        point,
      };
    });

    // Sort by depth (farthest first, drawn back-to-front)
    projected.sort((a, b) => a.depth - b.depth);

    for (const item of projected) {
      const point = item.point;
      const x = item.x;
      const y = item.y;
      const lift = (point.y - data.bounds.min[1]) / spanY;
      const radiusX = Math.min(34, Math.max(1.8, point.scaleX * scale * 1.8));
      const radiusZ = Math.min(34, Math.max(1.8, point.scaleZ * scale * 1.8));
      const angle = yawFromQuaternion(point.rotation) - rotation.yaw;
      const alpha = Math.min(0.68, Math.max(0.08, point.opacity * 0.62));
      const red = colorByte(point.r, lift);
      const green = colorByte(point.g, lift);
      const blue = colorByte(point.b, lift);
      ctx.save();
      ctx.translate(x, y);
      ctx.rotate(angle);
      ctx.scale(radiusX, radiusZ);
      const gradient = ctx.createRadialGradient(0, 0, 0, 0, 0, 1);
      gradient.addColorStop(0, `rgba(${red}, ${green}, ${blue}, ${alpha})`);
      gradient.addColorStop(0.55, `rgba(${red}, ${green}, ${blue}, ${alpha * 0.32})`);
      gradient.addColorStop(1, `rgba(${red}, ${green}, ${blue}, 0)`);
      ctx.fillStyle = gradient;
      ctx.beginPath();
      ctx.arc(0, 0, 1, 0, Math.PI * 2);
      ctx.fill();
      ctx.restore();
    }
  }, [data, rotation]);

  return (
    <canvas
      ref={canvasRef}
      className="preview-canvas"
      aria-label="Real Gaussian splat preview"
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={handlePointerUp}
      onPointerLeave={handlePointerUp}
      style={{ cursor: isDragging ? 'grabbing' : 'grab', touchAction: 'none' }}
    />
  );
}

function colorByte(channel: number, lift: number): number {
  return Math.round(Math.min(255, Math.max(0, channel * 220 + 35 * lift)));
}

function formatBounds(bounds: PreviewBounds): string {
  const min = bounds.min.map((value) => value.toFixed(2)).join(', ');
  const max = bounds.max.map((value) => value.toFixed(2)).join(', ');
  return `[${min}] / [${max}]`;
}

function yawFromQuaternion(rotation: [number, number, number, number]): number {
  const [w, x, y, z] = rotation;
  return Math.atan2(2 * (w * y + x * z), 1 - 2 * (y * y + z * z));
}
