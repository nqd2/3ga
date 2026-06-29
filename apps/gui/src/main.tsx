import React, { useEffect, useMemo, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import * as pc from 'playcanvas';
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
import './styles.css';

type Manifest = {
version: number;
source: { format: string; splatCount: number; keptCount: number };
artifacts: { manifest: string; collisionMeshJson: string };
metrics: Record<string, number | string | null>;
};

type Bounds = {
min: { x: number; y: number; z: number };
max: { x: number; y: number; z: number };
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
onPick
<dl>
<dt>Loaded</dt>
<dd>{sourceMetadata.format}</dd>
<dt>Rows</dt>
<dd>{sourceMetadata.splatCount}</dd>
</dl>
)}
{manifest && (
<dl>
<dt>Format</dt>
<dd>{manifest.source.format}</dd>
<dt>Splats</dt>
<dd>{manifest.source.splatCount}</dd>
<dt>Triangles</dt>
<dd>{manifest.metrics.collisionTrianglesAfterMerge}</dd>
</dl>
)}
<button disabled={!manifest || isProcessing} onClick={saveBundle}>
Save ZIP
</button>
</aside>
</section>
</main>
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
<div className="point-row" key={`${title}-${pointIndex}`}>
{point.map((value, axis) => (
<input
aria-label={`${title} ${pointIndex + 1} ${['x', 'y', 'z'][axis]}`}
key={axis}
type="number"
step="0.001"
value={value}
onChange={(event) => {
const next: Point3 = [...point] as Point3;
next[axis] = Number(event.target.value);
onChange(pointIndex, next);
}}
/>
))}
</div>
))}
</div>
);
}

createRoot(document.getElementById('root')!).render(<App />);
