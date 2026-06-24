import { Download, Package, Play, Save } from 'lucide-react';
import { useState } from 'react';
import type { EditRecipe } from '../edit/EditStore';
import { artifactUrl, submitJob, type JobResponse, type ProcessingConfig } from '../api/client';
import { downloadDemoZip } from '../demo/demoPackage';
import { open } from '@tauri-apps/plugin-dialog';

type JobPanelProps = {
  inputPath: string;
  outputDir: string;
  configPath: string;
  config: ProcessingConfig;
  editRecipePath: string;
  recipe: EditRecipe;
  job: JobResponse | null;
  busy: boolean;
  error: string | null;
  onInputPath(inputPath: string): void;
  onOutputDir(outputDir: string): void;
  onConfigPath(configPath: string): void;
  onEditRecipePath(editRecipePath: string): void;
  onJob(job: JobResponse): void;
  onBusy(busy: boolean): void;
  onError(error: string | null): void;
};

export function JobPanel(props: JobPanelProps) {
  const [packaging, setPackaging] = useState(false);
  const [packageError, setPackageError] = useState<string | null>(null);

  const isTauri = typeof window !== 'undefined' && !!(window as any).__TAURI_INTERNALS__;

  const pickInputPath = async () => {
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        filters: [{ name: 'Gaussian Splat / PLY', extensions: ['ply', 'splat', 'sog'] }]
      });
      if (selected && typeof selected === 'string') {
        props.onInputPath(selected);
      }
    } catch (err) {
      console.error('Failed to open input path picker:', err);
    }
  };

  const pickOutputDir = async () => {
    try {
      const selected = await open({
        multiple: false,
        directory: true
      });
      if (selected && typeof selected === 'string') {
        props.onOutputDir(selected);
      }
    } catch (err) {
      console.error('Failed to open output directory picker:', err);
    }
  };

  const pickConfigPath = async () => {
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        filters: [{ name: 'Config JSON / YAML', extensions: ['json', 'yaml', 'yml'] }]
      });
      if (selected && typeof selected === 'string') {
        props.onConfigPath(selected);
      }
    } catch (err) {
      console.error('Failed to open config picker:', err);
    }
  };

  const pickEditRecipePath = async () => {
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        filters: [{ name: 'Recipe JSON', extensions: ['json'] }]
      });
      if (selected && typeof selected === 'string') {
        props.onEditRecipePath(selected);
      }
    } catch (err) {
      console.error('Failed to open recipe picker:', err);
    }
  };

  const startJob = async () => {
    props.onBusy(true);
    props.onError(null);
    try {
      const job = await submitJob({
        inputPath: props.inputPath,
        outputDir: props.outputDir,
        configPath: props.configPath,
        config: props.config,
        editRecipePath: props.editRecipePath,
        editRecipe: props.editRecipePath ? undefined : props.recipe,
      });
      props.onJob(job);
    } catch (error) {
      props.onError(error instanceof Error ? error.message : 'job failed');
    } finally {
      props.onBusy(false);
    }
  };

  const downloadPackage = async () => {
    if (!props.job) {
      return;
    }
    setPackaging(true);
    setPackageError(null);
    try {
      await downloadDemoZip(props.job);
    } catch (error) {
      setPackageError(error instanceof Error ? error.message : 'demo ZIP failed');
    } finally {
      setPackaging(false);
    }
  };

  return (
    <section className="panel job-panel" aria-label="Processing job">
      <div className="panel__title">
        <span>Job</span>
        <code>{props.job?.state ?? 'idle'}</code>
      </div>
      <label>
        Input path
        <div style={{ display: 'flex', gap: '8px' }}>
          <input value={props.inputPath} onChange={(event) => props.onInputPath(event.target.value)} />
          {isTauri && (
            <button type="button" onClick={pickInputPath} style={{ flexShrink: 0, padding: '0 12px', border: '1px solid var(--line)', borderRadius: '6px', cursor: 'pointer', background: 'var(--panel)' }}>
              Browse...
            </button>
          )}
        </div>
      </label>
      <label>
        Output dir
        <div style={{ display: 'flex', gap: '8px' }}>
          <input value={props.outputDir} onChange={(event) => props.onOutputDir(event.target.value)} />
          {isTauri && (
            <button type="button" onClick={pickOutputDir} style={{ flexShrink: 0, padding: '0 12px', border: '1px solid var(--line)', borderRadius: '6px', cursor: 'pointer', background: 'var(--panel)' }}>
              Browse...
            </button>
          )}
        </div>
      </label>
      <label>
        Config
        <div style={{ display: 'flex', gap: '8px' }}>
          <input value={props.configPath} onChange={(event) => props.onConfigPath(event.target.value)} />
          {isTauri && (
            <button type="button" onClick={pickConfigPath} style={{ flexShrink: 0, padding: '0 12px', border: '1px solid var(--line)', borderRadius: '6px', cursor: 'pointer', background: 'var(--panel)' }}>
              Browse...
            </button>
          )}
        </div>
      </label>
      <label>
        Edits path
        <div style={{ display: 'flex', gap: '8px' }}>
          <input value={props.editRecipePath} onChange={(event) => props.onEditRecipePath(event.target.value)} />
          {isTauri && (
            <button type="button" onClick={pickEditRecipePath} style={{ flexShrink: 0, padding: '0 12px', border: '1px solid var(--line)', borderRadius: '6px', cursor: 'pointer', background: 'var(--panel)' }}>
              Browse...
            </button>
          )}
        </div>
      </label>
      <button className="primary-action" type="button" disabled={props.busy || !props.inputPath || !props.outputDir} onClick={startJob}>
        <Play size={17} />
        Process
      </button>
      {props.error ? <p className="job-panel__error">{props.error}</p> : null}
      {packageError ? <p className="job-panel__error">{packageError}</p> : null}
      <div className="job-panel__recipe">
        <div>
          <Save size={16} />
          <span>{props.recipe.operations.length}</span>
        </div>
        <pre>{JSON.stringify(props.recipe, null, 2)}</pre>
      </div>
      {props.job ? (
        <div className="artifacts">
          {Object.keys(props.job.artifacts).map((name) => (
            <a key={name} href={artifactUrl(props.job!.id, name)}>
              <Download size={15} />
              {name}
            </a>
          ))}
          <button type="button" disabled={props.job.state !== 'done' || packaging} onClick={downloadPackage}>
            <Package size={15} />
            demo zip
          </button>
        </div>
      ) : null}
    </section>
  );
}
