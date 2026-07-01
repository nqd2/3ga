import React from 'react';
import { Button } from '@astryxdesign/core/Button';
import { Play, Square } from 'lucide-react';

export function Header({
  inputPath,
  sourceMetadata,
  isBusy,
  isCalibrationValid,
  loadSource,
  runProcess,
  cancelJob,
}: {
  inputPath: string;
  sourceMetadata: any;
  isBusy: boolean;
  isCalibrationValid: boolean;
  loadSource: () => void;
  runProcess: () => void;
  cancelJob: () => void;
}) {
  return (
    <header className="toolbar">
      <div>
        <p className="eyebrow">3DGS to AR geometry</p>
        <h1>augmented-gaussian</h1>
      </div>
      <div className="toolbar-actions">
        <Button
          label="Load Source"
          variant={sourceMetadata ? "secondary" : "primary"}
          isDisabled={!inputPath || isBusy}
          onClick={loadSource}
        />
        <Button
          label="Bake Geometry"
          variant={isCalibrationValid ? "primary" : "secondary"}
          isDisabled={!isCalibrationValid || isBusy}
          onClick={runProcess}
          icon={<Play size={14} />}
          tooltip={!isCalibrationValid ? "Complete calibration first" : undefined}
        />
        <Button
          label="Cancel Job"
          variant="destructive"
          isDisabled={!isBusy}
          onClick={cancelJob}
          icon={<Square size={12} />}
        />
      </div>
    </header>
  );
}
