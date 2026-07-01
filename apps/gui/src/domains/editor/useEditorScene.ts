import { useCallback, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useLocalStorage } from '../../hooks/useLocalStorage';
import {
  defaultSceneTransform,
  isDefaultSceneTransform,
  updateTransformAxis,
  type EditorSceneState,
  type EditorSourceExport,
  type SceneTransform,
} from './sceneTransform';
import { type Bounds } from '../../domains/calibration';

export type EditorView = 'editor' | 'bake';
export type CameraMode = 'orbit' | 'fly';

export function useEditorScene({
  sourcePath,
  splatCount,
  bounds,
}: {
  sourcePath: string | null;
  splatCount: number;
  bounds: Bounds | null;
}) {
  const [activeView, setActiveView] = useLocalStorage<EditorView>('ag_editor_active_view', 'bake');
  const [cameraMode, setCameraMode] = useLocalStorage<CameraMode>('ag_editor_camera_mode', 'orbit');
  const [visible, setVisible] = useLocalStorage<boolean>('ag_editor_scene_visible', true);
  const [deleted, setDeleted] = useLocalStorage<boolean>('ag_editor_scene_deleted', false);
  const [transform, setTransform] =
    useLocalStorage<SceneTransform>('ag_editor_scene_transform', defaultSceneTransform);
  const [lastExport, setLastExport] =
    useLocalStorage<EditorSourceExport | null>('ag_editor_last_export', null);

  const sceneState: EditorSceneState = useMemo(() => ({
    sourcePath,
    visible,
    deleted,
    transform,
    splatCount,
    bounds,
  }), [bounds, deleted, sourcePath, splatCount, transform, visible]);

  const setTransformAxis = useCallback((
    key: keyof SceneTransform,
    index: number,
    value: number,
  ) => {
    setTransform((prev) => updateTransformAxis(prev, key, index, value));
  }, [setTransform]);

  const resetTransform = useCallback(() => {
    setTransform(defaultSceneTransform);
  }, [setTransform]);

  const resetScene = useCallback(() => {
    setVisible(true);
    setDeleted(false);
    setTransform(defaultSceneTransform);
    setLastExport(null);
  }, [setDeleted, setLastExport, setTransform, setVisible]);

  const exportVisibleMergedSource = useCallback(async () => {
    if (!sourcePath) throw new Error('No source loaded');
    const output = await invoke<EditorSourceExport>('export_edited_source', {
      request: {
        inputPath: sourcePath,
        transform,
        visible,
        deleted,
      },
    });
    setLastExport(output);
    return output;
  }, [deleted, setLastExport, sourcePath, transform, visible]);

  return {
    activeView,
    setActiveView,
    cameraMode,
    setCameraMode,
    visible,
    setVisible,
    deleted,
    setDeleted,
    transform,
    setTransform,
    setTransformAxis,
    resetTransform,
    resetScene,
    sceneState,
    lastExport,
    exportVisibleMergedSource,
    isTransformDirty: !isDefaultSceneTransform(transform),
  };
}
