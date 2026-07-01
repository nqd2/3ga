import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { type SourceMetadata } from '../components/Preview';
import { type ViewerLoadProgress, type ViewerSourceSummary } from '../classes/PlayCanvasViewer';
import { useLocalStorage } from './useLocalStorage';

function isRawSplatUrl(url: string) {
  return url.split(/[?#]/, 1)[0].toLowerCase().endsWith('.splat');
}

function sourcePathToUrl(path: string) {
  try {
    return convertFileSrc(path);
  } catch {
    return null;
  }
}

export function useSourceLoader({ setStatus }: { setStatus: (s: string) => void }) {
  const [inputPath, setInputPath] = useLocalStorage<string>('ag_input_path', '');
  const [sourceUrl, setSourceUrl] = useState<string | null>(null);
  const [sourceMetadata, setSourceMetadata] = useState<SourceMetadata | null>(null);
  const [isSourceLoading, setIsSourceLoading] = useState(false);
  const [sourceLoadProgress, setSourceLoadProgress] = useState<ViewerLoadProgress | null>(null);
  const sourceLoadTokenRef = useRef(0);

  const isSourceReady = useMemo(() => {
    return Boolean(sourceMetadata?.bounds);
  }, [sourceMetadata]);

  const handleBrowseInputPath = useCallback(async () => {
    const selected = await open({
      multiple: false,
      filters: [{ name: 'Point Cloud source', extensions: ['ply', 'splat'] }],
    });
    if (typeof selected === 'string') {
      setInputPath(selected);
    }
  }, [setInputPath]);

  const handleSourceProgress = useCallback((progress: ViewerLoadProgress) => {
    setSourceLoadProgress(progress);
  }, []);

  const handleSourceReady = useCallback((summary: ViewerSourceSummary) => {
    setIsSourceLoading(false);
    setSourceLoadProgress(null);
    setStatus('source loaded');
    setSourceMetadata({
      path: inputPath,
      bytes: 0,
      format: isRawSplatUrl(inputPath) ? 'splat' : 'ply',
      splatCount: summary.splatCount,
      bounds: summary.bounds,
    });
  }, [inputPath, setStatus]);

  const handleSourceError = useCallback((err: Error) => {
    setIsSourceLoading(false);
    setSourceLoadProgress(null);
    setStatus(`error: ${err.message}`);
  }, [setStatus]);

  const loadSource = useCallback(async () => {
    if (!inputPath) return;
    const token = ++sourceLoadTokenRef.current;
    setIsSourceLoading(true);
    setSourceLoadProgress(null);
    setSourceMetadata(null);
    setSourceUrl(null);
    setStatus('loading source');

    try {
      const metadata = await invoke<SourceMetadata>('load_source', { path: inputPath });
      if (sourceLoadTokenRef.current !== token) return;

      const canonicalUrl = sourcePathToUrl(metadata.previewPath || metadata.path);
      if (!canonicalUrl) {
        throw new Error('failed to convert file source path to safe browser URL');
      }

      setSourceUrl(canonicalUrl);
      setSourceMetadata(metadata);
    } catch (err) {
      if (sourceLoadTokenRef.current !== token) return;
      setIsSourceLoading(false);
      setStatus(`error: ${err instanceof Error ? err.message : String(err)}`);
    }
  }, [inputPath, setStatus]);

  const cancelLoad = useCallback(() => {
    setIsSourceLoading(false);
    setSourceLoadProgress(null);
    setSourceUrl(null);
    setSourceMetadata(null);
    setStatus('source load cancelled');
  }, [setStatus]);

  // Auto-reload point cloud on mount if inputPath has been saved
  useEffect(() => {
    if (inputPath) {
      loadSource();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return {
    inputPath,
    setInputPath,
    handleBrowseInputPath,
    sourceUrl,
    sourceMetadata,
    isSourceLoading,
    sourceLoadProgress,
    isSourceReady,
    loadSource,
    cancelLoad,
    handleSourceProgress,
    handleSourceReady,
    handleSourceError,
  };
}
