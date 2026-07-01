import React from 'react';
import { Button } from '@astryxdesign/core/Button';
import { Badge } from '@astryxdesign/core/Badge';
import { NumberInput } from '@astryxdesign/core/NumberInput';
import { Check } from 'lucide-react';
import { type Point3, type PickMode } from '../domains/calibration';

export function PointEditor({
  title,
  points,
  pickModePrefix,
  currentPickMode,
  userPickedStates,
  onSelectPickMode,
  onChange,
}: {
  title: string;
  points: readonly Point3[];
  pickModePrefix: string;
  currentPickMode: PickMode;
  userPickedStates: readonly boolean[];
  onSelectPickMode: (mode: PickMode) => void;
  onChange: (index: number, point: Point3) => void;
}) {
  return (
    <div className="point-editor">
      {points.map((point, pointIndex) => {
        const pointPickMode = `${pickModePrefix}${pointIndex}` as PickMode;
        const isActive = currentPickMode === pointPickMode;
        const isPicked = userPickedStates[pointIndex];
        const pointName = `${title.replace('endpoints', '').replace('points', '').trim()} Point ${pointIndex + 1}`;

        return (
          <div key={`${title}-${pointIndex}`} style={{ marginBottom: '12px' }}>
            <div className="point-row-header" style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: '8px', marginBottom: '6px' }}>
              <span className="point-row-label">{pointName}</span>
              <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                <Button
                  label={isActive ? "Picking" : "Pick"}
                  variant={isActive ? "primary" : "ghost"}
                  size="sm"
                  onClick={() => onSelectPickMode(pointPickMode)}
                />
                <Badge
                  variant={isPicked ? "success" : "info"}
                  label={isPicked ? "Selected" : "Pending"}
                  icon={isPicked ? <Check size={10} /> : undefined}
                />
              </div>
            </div>
            <div className="point-row">
              {point.map((value, axis) => (
                <NumberInput
                  key={axis}
                  label={`${title} ${pointIndex + 1} ${['x', 'y', 'z'][axis]}`}
                  isLabelHidden
                  step={0.01}
                  value={value}
                  onChange={(val: number | null | undefined) => {
                    const next: Point3 = [...point] as Point3;
                    next[axis] = val || 0;
                    onChange(pointIndex, next);
                  }}
                />
              ))}
            </div>
          </div>
        );
      })}
    </div>
  );
}
