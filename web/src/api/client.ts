import type { EditRecipe } from '../edit/EditStore';
import { invoke, convertFileSrc } from '@tauri-apps/api/core';

// Cache for job artifacts in desktop mode
const jobArtifactsCache = new Map<string, Record<string, string>>();

export type JobResponse = {
  id: string;
  state: string;
  states: string[];
  artifacts: Record<string, string>;
  error?: string | null;
};

export type ProcessingConfig = {
  voxelSize: number;
  opacityCutoff: number;
  navmesh: {
    agentRadius: number;
    agentHeight: number;
  };
};

export type SubmitJobInput = {
  inputPath: string;
  outputDir: string;
  configPath?: string;
  config?: ProcessingConfig;
  editRecipePath?: string;
  editRecipe?: EditRecipe;
};

const API_BASE = import.meta.env.VITE_GA3D_API_BASE ?? '';

export async function submitJob(input: SubmitJobInput): Promise<JobResponse> {
  // Check if running in Tauri desktop environment
  if (typeof window !== 'undefined' && (window as any).__TAURI_INTERNALS__) {
    try {
      const result = await invoke<JobResponse>('run_job', {
        inputPath: input.inputPath,
        outputDir: input.outputDir,
        config: input.config || null,
        editRecipe: input.editRecipe || null,
      });
      // Cache artifacts paths for URL retrieval
      if (result && result.artifacts) {
        jobArtifactsCache.set(result.id, result.artifacts);
      }
      return result;
    } catch (err) {
      throw new Error(String(err));
    }
  }

  // Fallback to web-server HTTP fetch
  const response = await fetch(`${API_BASE}/api/jobs`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      input_path: input.inputPath,
      output_dir: input.outputDir,
      config_path: input.configPath || null,
      config: input.configPath ? null : (input.config ?? null),
      edit_recipe_path: input.editRecipePath || null,
      edit_recipe: input.editRecipe ?? null,
    }),
  });
  if (!response.ok) {
    throw new Error(`job submit failed: ${response.status} ${await response.text()}`);
  }
  return response.ok ? response.json() : response; // safe type assertion
}

export function artifactUrl(jobId: string, name: string): string {
  if (typeof window !== 'undefined' && (window as any).__TAURI_INTERNALS__) {
    const artifacts = jobArtifactsCache.get(jobId);
    if (artifacts && artifacts[name]) {
      return convertFileSrc(artifacts[name]);
    }
  }
  return `${API_BASE}/api/jobs/${jobId}/artifacts/${name}`;
}
