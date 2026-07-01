import { type Bounds } from '../domains/calibration';

export type Manifest = {
  version: number;
  source: {
    format: string;
    splatCount: number;
    keptCount: number;
    originalPath?: string;
    editedPath?: string;
    editedSplatCount?: number;
    editedBytes?: number;
  };
  bounds?: Bounds | null;
  artifacts: { manifest: string; collisionMeshJson: string };
  metrics: Record<string, unknown>;
};

export type ProcessRequest = {
  inputPath: string;
  outDir: string;
  configJson: string;
  recipeJson: string;
  sourceContext?: {
    originalPath: string;
    editedPath: string;
    editedSplatCount: number;
    editedBytes: number;
  };
};

export type JobProgress = { stage: string };

export const stageProgress: Record<string, number> = {
  decode: 12,
  alignment: 24,
  filters: 34,
  voxelize: 48,
  fill: 58,
  carve: 68,
  mesh: 80,
  navmesh: 90,
  export: 96,
  done: 100,
};

export function getStageName(status: string) {
  if (status === 'loading source' || status.startsWith('loading source')) return 'decode';
  if (status === 'processing') return 'decode';
  if (status.startsWith('processing: ')) return status.replace('processing: ', '');
  if (status === 'done') return 'done';
  return 'idle';
}

export function getStageLabel(stage: string) {
  if (stage === 'alignment') return 'align';
  if (stage === 'filters') return 'filter';
  return stage;
}
