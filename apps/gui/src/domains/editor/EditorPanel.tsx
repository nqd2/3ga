import React from 'react';
import { Badge } from '@astryxdesign/core/Badge';
import { Button } from '@astryxdesign/core/Button';
import { Card } from '@astryxdesign/core/Card';
import { TextInput } from '@astryxdesign/core/TextInput';
import { Eye, EyeOff, FolderOpen, MousePointer2, Move3D, Plane, Rotate3D, Trash2 } from 'lucide-react';
import { SceneTransformControls } from './SceneTransformControls';
import { type CameraMode } from './useEditorScene';
import { type EditorSceneState, type SceneTransform } from './sceneTransform';

export function EditorPanel({
  inputPath,
  setInputPath,
  handleBrowseInputPath,
  scene,
  cameraMode,
  setCameraMode,
  setSceneVisible,
  deleteScene,
  resetScene,
  setTransformAxis,
  resetTransform,
  isBusy,
}: {
  inputPath: string;
  setInputPath: (value: string) => void;
  handleBrowseInputPath: () => void;
  scene: EditorSceneState;
  cameraMode: CameraMode;
  setCameraMode: (mode: CameraMode) => void;
  setSceneVisible: (value: boolean) => void;
  deleteScene: () => void;
  resetScene: () => void;
  setTransformAxis: (key: keyof SceneTransform, index: number, value: number) => void;
  resetTransform: () => void;
  isBusy: boolean;
}) {
  const hasScene = Boolean(scene.sourcePath) && !scene.deleted;
  return (
    <aside className="panel">
      <Card variant="default" padding={4}>
        <div className="card-stack">
          <div className="config-group">
            <h3 className="config-group-title">Editor Source</h3>
            <div className="picker-row">
              <TextInput
                label="Source PLY/SPLAT Path"
                value={inputPath}
                onChange={(value: string) => setInputPath(value)}
                status={inputPath ? { type: 'success' } : undefined}
              />
              <Button
                label="Browse"
                isIconOnly
                icon={<FolderOpen size={16} />}
                onClick={handleBrowseInputPath}
                tooltip="Browse Source PLY/SPLAT"
              />
            </div>
          </div>

          <div className="config-group">
            <div className="section-title-row">
              <h3 className="config-group-title">Scene</h3>
              <Badge
                variant={hasScene && scene.visible ? 'success' : 'neutral'}
                label={hasScene ? `${scene.splatCount.toLocaleString()} splats` : 'empty'}
              />
            </div>
            <div className="editor-button-grid">
              <Button
                label={scene.visible ? 'Visible' : 'Hidden'}
                variant={scene.visible ? 'secondary' : 'primary'}
                icon={scene.visible ? <Eye size={14} /> : <EyeOff size={14} />}
                isDisabled={!hasScene || isBusy}
                onClick={() => setSceneVisible(!scene.visible)}
              />
              <Button
                label="Delete"
                variant="destructive"
                icon={<Trash2 size={14} />}
                isDisabled={!hasScene || isBusy}
                onClick={deleteScene}
              />
              <Button
                label="Reset Scene"
                variant="secondary"
                isDisabled={isBusy}
                onClick={resetScene}
              />
            </div>
          </div>

          <div className="config-group">
            <h3 className="config-group-title">Tools</h3>
            <div className="editor-button-grid">
              <Button label="Select" variant="secondary" icon={<MousePointer2 size={14} />} />
              <Button label="Move" variant="secondary" icon={<Move3D size={14} />} />
              <Button label="Rotate" variant="secondary" icon={<Rotate3D size={14} />} />
              <Button label="Floor Pick" variant="secondary" icon={<Plane size={14} />} />
            </div>
          </div>

          <div className="config-group">
            <h3 className="config-group-title">Camera</h3>
            <div className="editor-button-grid">
              <Button
                label="Orbit"
                variant={cameraMode === 'orbit' ? 'primary' : 'secondary'}
                onClick={() => setCameraMode('orbit')}
              />
              <Button
                label="WASD Fly"
                variant={cameraMode === 'fly' ? 'primary' : 'secondary'}
                onClick={() => setCameraMode('fly')}
              />
            </div>
          </div>

          <SceneTransformControls
            transform={scene.transform}
            setTransformAxis={setTransformAxis}
            resetTransform={resetTransform}
            isDisabled={isBusy || !hasScene}
          />
        </div>
      </Card>
    </aside>
  );
}
