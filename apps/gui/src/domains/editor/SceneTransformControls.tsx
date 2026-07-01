import React from 'react';
import { Button } from '@astryxdesign/core/Button';
import { NumberInput } from '@astryxdesign/core/NumberInput';
import { RotateCcw } from 'lucide-react';
import { type SceneTransform } from './sceneTransform';

export function SceneTransformControls({
  transform,
  setTransformAxis,
  resetTransform,
  isDisabled,
}: {
  transform: SceneTransform;
  setTransformAxis: (key: keyof SceneTransform, index: number, value: number) => void;
  resetTransform: () => void;
  isDisabled?: boolean;
}) {
  return (
    <div className="config-group">
      <div className="section-title-row">
        <h3 className="config-group-title">Scene Transform</h3>
        <Button
          label="Reset"
          variant="secondary"
          size="sm"
          icon={<RotateCcw size={12} />}
          isDisabled={isDisabled}
          onClick={resetTransform}
        />
      </div>
      <AxisRow
        label="Position XYZ"
        values={transform.position}
        step={0.05}
        units="m"
        isDisabled={isDisabled}
        onChange={(index, value) => setTransformAxis('position', index, value)}
      />
      <AxisRow
        label="Rotation XYZ"
        values={transform.rotationEulerDeg}
        step={1}
        units="deg"
        isDisabled={isDisabled}
        onChange={(index, value) => setTransformAxis('rotationEulerDeg', index, value)}
      />
    </div>
  );
}

function AxisRow({
  label,
  values,
  step,
  units,
  isDisabled,
  onChange,
}: {
  label: string;
  values: [number, number, number];
  step: number;
  units: string;
  isDisabled?: boolean;
  onChange: (index: number, value: number) => void;
}) {
  return (
    <div className="axis-editor">
      <span className="point-row-label">{label}</span>
      <div className="point-row">
        {(['X', 'Y', 'Z'] as const).map((axis, index) => (
          <NumberInput
            key={axis}
            label={`${label} ${axis}`}
            step={step}
            units={units}
            value={values[index]}
            isDisabled={isDisabled}
            onChange={(value: number | null | undefined) => onChange(index, value ?? 0)}
          />
        ))}
      </div>
    </div>
  );
}
