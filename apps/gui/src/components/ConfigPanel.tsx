import React from 'react';
import { Card } from '@astryxdesign/core/Card';
import { TextInput } from '@astryxdesign/core/TextInput';
import { Button } from '@astryxdesign/core/Button';
import { NumberInput } from '@astryxdesign/core/NumberInput';
import { FolderOpen } from 'lucide-react';
import { E2eSelector } from './E2eSelector';
import { PointEditor } from './PointEditor';
import { SceneTransformControls } from '../domains/editor/SceneTransformControls';
import { type SceneTransform } from '../domains/editor/sceneTransform';
import { type GeometryProfile, type Point3, type PickMode, type UpAxis } from '../domains/calibration';

export function ConfigPanel({
  inputPath,
  setInputPath,
  handleBrowseInputPath,
  outDir,
  setOutDir,
  handleBrowseOutDir,
  distance,
  setDistance,
  pickMode,
  setPickMode,
  upAxis,
  setUpAxis,
  geometryProfile,
  setGeometryProfile,
  sceneTransform,
  setSceneTransformAxis,
  resetSceneTransform,
  scalePoints,
  updateScalePoint,
  userPickedScalePoints,
  isProcessing,
}: {
  inputPath: string;
  setInputPath: (val: string) => void;
  handleBrowseInputPath: () => void;
  outDir: string;
  setOutDir: (val: string) => void;
  handleBrowseOutDir: () => void;
  distance: number;
  setDistance: (val: number) => void;
  pickMode: PickMode;
  setPickMode: (val: PickMode) => void;
  upAxis: UpAxis;
  setUpAxis: (val: UpAxis) => void;
  geometryProfile: GeometryProfile;
  setGeometryProfile: (val: GeometryProfile) => void;
  sceneTransform: SceneTransform;
  setSceneTransformAxis: (key: keyof SceneTransform, index: number, value: number) => void;
  resetSceneTransform: () => void;
  scalePoints: [Point3, Point3];
  updateScalePoint: (idx: number, pt: Point3) => void;
  userPickedScalePoints: [boolean, boolean];
  isProcessing: boolean;
}) {
  return (
    <aside className="panel">
      <Card variant="default" padding={4}>
        <div className="card-stack">
          <div className="config-group">
            <h3 className="config-group-title">Source</h3>
            <div className="config-group-fields">
              <div className="picker-row">
                <TextInput
                  label="Source PLY/SPLAT Path"
                  value={inputPath}
                  onChange={(val: string) => setInputPath(val)}
                  placeholder="/path/to/file.ply"
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
              <div className="picker-row">
                <TextInput
                  label="Output Directory"
                  value={outDir}
                  onChange={(val: string) => setOutDir(val)}
                  status={outDir ? { type: 'success' } : undefined}
                />
                <Button
                  label="Browse"
                  isIconOnly
                  icon={<FolderOpen size={16} />}
                  onClick={handleBrowseOutDir}
                  tooltip="Browse Output Directory"
                />
              </div>
            </div>
          </div>

          <SceneTransformControls
            transform={sceneTransform}
            setTransformAxis={setSceneTransformAxis}
            resetTransform={resetSceneTransform}
            isDisabled={isProcessing}
          />

          <div className="config-group">
            <h3 className="config-group-title">Calibration</h3>
            <div className="config-group-fields">
              <NumberInput
                label="Scale Calibration Distance"
                min={0.001}
                step={0.1}
                units="m"
                value={distance}
                onChange={(val: number | null | undefined) => setDistance(val ?? 2)}
                status={distance > 0 ? { type: 'success' } : { type: 'error', message: 'Distance must be positive.' }}
              />
              <E2eSelector
                label="Pick Target"
                astryxLabel="Selection Target"
                options={[
                  { value: 'scale0', label: 'Scale Endpoint 1' },
                  { value: 'scale1', label: 'Scale Endpoint 2' },
                ]}
                value={pickMode}
                onChange={(val: string) => setPickMode(val as PickMode)}
                status={{ type: 'success' }}
              />
              <E2eSelector
                label="Up Axis"
                astryxLabel="Up Direction"
                options={[
                  { value: 'y', label: 'Y up' },
                  { value: 'z', label: 'Z up' },
                  { value: 'x', label: 'X up' },
                  { value: 'neg-y', label: '-Y up' },
                  { value: 'neg-z', label: '-Z up' },
                  { value: 'neg-x', label: '-X up' },
                ]}
                value={upAxis}
                onChange={(val: string) => setUpAxis(val as UpAxis)}
                status={{ type: 'success' }}
              />
              <E2eSelector
                label="Bake Profile"
                astryxLabel="Collision Pipeline"
                options={[
                  { value: 'object-prop', label: 'Object / prop' },
                  { value: 'interior-room', label: 'Interior room' },
                  { value: 'outdoor-terrain', label: 'Outdoor terrain' },
                ]}
                value={geometryProfile}
                onChange={(val: string) => setGeometryProfile(val as GeometryProfile)}
                status={{ type: 'success' }}
              />
            </div>
          </div>

          <PointEditor
            title="Scale endpoints"
            points={scalePoints}
            pickModePrefix="scale"
            currentPickMode={pickMode}
            userPickedStates={userPickedScalePoints}
            onSelectPickMode={setPickMode}
            onChange={updateScalePoint}
          />
        </div>
      </Card>
    </aside>
  );
}
