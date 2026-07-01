import { useCallback, useEffect, useMemo, useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getStageName, type Manifest, type ProcessRequest, type JobProgress } from '../utils/jobHelpers';
import { useLocalStorage } from './useLocalStorage';

type PreparedBakeSource = {
  inputPath: string;
  sourceContext?: ProcessRequest['sourceContext'];
};

export function useBakeJob({
  status,
  setStatus,
  inputPath,
  recipe,
  config,
  isCalibrationValid,
  prepareBakeSource,
}: {
  status: string;
  setStatus: (s: string) => void;
  inputPath: string;
  recipe: any;
  config: any;
  isCalibrationValid: boolean;
  prepareBakeSource?: () => Promise<PreparedBakeSource>;
}) {
  const [outDir, setOutDir] = useLocalStorage<string>('ag_bake_out_dir', '~/Downloads/augmented-gaussian');
  const [isProcessing, setIsProcessing] = useState(false);
  const [manifest, setManifest] = useLocalStorage<Manifest | null>('ag_bake_manifest', null);
  const [artifactOutDir, setArtifactOutDir] = useLocalStorage<string | null>('ag_bake_artifact_out_dir', null);

  const progressStage = useMemo(() => getStageName(status), [status]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen<JobProgress>('job-progress', (event) => {
      setStatus(`processing: ${event.payload.stage}`);
    })
      .then((handler) => {
        unlisten = handler;
      })
      .catch(() => { });
    return () => unlisten?.();
  }, [setStatus]);

  const handleBrowseOutDir = useCallback(async () => {
    const selected = await open({
      multiple: false,
      directory: true,
    });
    if (typeof selected === 'string') {
      setOutDir(selected);
    }
  }, [setOutDir]);

  const runProcess = useCallback(async () => {
    if (!isCalibrationValid || isProcessing) return;
    setIsProcessing(true);
    setManifest(null);
    setArtifactOutDir(null);
    setStatus('processing');

    try {
      const prepared = prepareBakeSource
        ? await prepareBakeSource()
        : { inputPath };
      const request: ProcessRequest = {
        inputPath: prepared.inputPath,
        outDir,
        configJson: JSON.stringify(config),
        recipeJson: JSON.stringify(recipe),
        sourceContext: prepared.sourceContext,
      };
      const output = await invoke<Manifest>('process_job', { request });
      setManifest(output);
      setArtifactOutDir(outDir);
      setStatus('done');
    } catch (err) {
      setStatus(`error: ${err instanceof Error ? err.message : String(err)}`);
    } finally {
      setIsProcessing(false);
    }
  }, [config, isCalibrationValid, isProcessing, inputPath, outDir, prepareBakeSource, recipe, setStatus, setManifest, setArtifactOutDir]);

  const cancelJob = useCallback(async () => {
    try {
      await invoke('cancel_job');
      setStatus('cancel requested');
    } catch (err) {
      setStatus(`error: ${err instanceof Error ? err.message : String(err)}`);
    }
  }, [setStatus]);

  const saveBundle = useCallback(async () => {
    if (!manifest) return;
    const bundleOutDir = artifactOutDir ?? outDir;
    const destinationPath = `${bundleOutDir}/webar-copy.zip`;
    try {
      const savedPath = await invoke<string>('save_bundle', {
        request: { outDir: bundleOutDir, destinationPath }
      });
      setStatus(`saved ${savedPath}`);
    } catch (err) {
      setStatus(`error: ${err instanceof Error ? err.message : String(err)}`);
    }
  }, [artifactOutDir, manifest, outDir, setStatus]);

  const previewWebAr = useCallback(async () => {
    if (!manifest) return;
    const bundleOutDir = artifactOutDir ?? outDir;
    const indexPath = `${bundleOutDir}/index.html`;
    try {
      await invoke('open_webar_viewer', { path: indexPath });
      setStatus('preview opened');
    } catch (err) {
      setStatus(`error: ${err instanceof Error ? err.message : String(err)}`);
    }
  }, [artifactOutDir, manifest, outDir, setStatus]);

  return {
    outDir,
    setOutDir,
    handleBrowseOutDir,
    isProcessing,
    manifest,
    runProcess,
    cancelJob,
    saveBundle,
    previewWebAr,
    progressStage,
  };
}
