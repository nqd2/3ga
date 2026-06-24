import { useMemo, useState } from 'react';
import { SplatViewer } from '../splat/SplatViewer';
import { createEditStore } from '../edit/EditStore';
import { EditToolbar } from '../edit/EditToolbar';
import { JobPanel } from '../jobs/JobPanel';
import type { JobResponse, ProcessingConfig } from '../api/client';

const DEFAULT_CONFIG: ProcessingConfig = {
  voxelSize: 0.1,
  opacityCutoff: 0.05,
  navmesh: {
    agentRadius: 0.2,
    agentHeight: 1.6,
  },
};

type ConfigField = 'voxelSize' | 'opacityCutoff' | 'agentRadius' | 'agentHeight';

export function App() {
  const store = useMemo(() => createEditStore(), []);
  const [, refresh] = useState(0);
  const [inputPath, setInputPath] = useState('tests/fixtures/minimal.ply');
  const [outputDir, setOutputDir] = useState('dist/job-001');
  const [configPath, setConfigPath] = useState('');
  const [config, setConfig] = useState<ProcessingConfig>(DEFAULT_CONFIG);
  const [editRecipePath, setEditRecipePath] = useState('');
  const [job, setJob] = useState<JobResponse | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const recipe = store.recipe;
  const touch = () => refresh((value) => value + 1);
  const setConfigNumber = (field: ConfigField, raw: string) => {
    const value = Number.parseFloat(raw);
    if (!Number.isFinite(value)) {
      return;
    }
    setConfig((current) => {
      if (field === 'agentRadius' || field === 'agentHeight') {
        return { ...current, navmesh: { ...current.navmesh, [field]: value } };
      }
      return { ...current, [field]: value };
    });
  };

  return (
    <main className="workspace">
      <header className="topbar">
        <div>
          <h1>ga3d</h1>
          <span>3DGS AR processing</span>
        </div>
        <div className="path-mode">
          {typeof window !== 'undefined' && (window as any).__TAURI_INTERNALS__ ? 'Desktop Native' : '/api proxy :8000'}
        </div>
      </header>

      <section className="workgrid">
        <div className="stage">
          <EditToolbar store={store} onChange={touch} />
          <SplatViewer inputPath={inputPath} recipe={recipe} onInputPathChange={setInputPath} />
        </div>

        <aside className="side">
          <section className="panel config-panel" aria-label="Processing config">
            <div className="panel__title">
              <span>Config</span>
              <code>webxr</code>
            </div>
            <div className="field-grid">
              <label>
                Voxel
                <input
                  value={config.voxelSize}
                  inputMode="decimal"
                  onChange={(event) => setConfigNumber('voxelSize', event.target.value)}
                />
              </label>
              <label>
                Opacity
                <input
                  value={config.opacityCutoff}
                  inputMode="decimal"
                  onChange={(event) => setConfigNumber('opacityCutoff', event.target.value)}
                />
              </label>
              <label>
                Agent radius
                <input
                  value={config.navmesh.agentRadius}
                  inputMode="decimal"
                  onChange={(event) => setConfigNumber('agentRadius', event.target.value)}
                />
              </label>
              <label>
                Agent height
                <input
                  value={config.navmesh.agentHeight}
                  inputMode="decimal"
                  onChange={(event) => setConfigNumber('agentHeight', event.target.value)}
                />
              </label>
            </div>
          </section>

          <JobPanel
            inputPath={inputPath}
            outputDir={outputDir}
            configPath={configPath}
            config={config}
            editRecipePath={editRecipePath}
            recipe={recipe}
            job={job}
            busy={busy}
            error={error}
            onInputPath={setInputPath}
            onOutputDir={setOutputDir}
            onConfigPath={setConfigPath}
            onEditRecipePath={setEditRecipePath}
            onJob={setJob}
            onBusy={setBusy}
            onError={setError}
          />
        </aside>
      </section>
    </main>
  );
}
