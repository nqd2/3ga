import React from 'react';
import { Selector } from '@astryxdesign/core/Selector';

export function E2eSelector({
  label,
  astryxLabel,
  options,
  value,
  onChange,
  status
}: {
  label: string;
  astryxLabel: string;
  options: { value: string; label: string }[];
  value: string;
  onChange: (val: string) => void;
  status?: { type: 'success' | 'warning' | 'error'; message?: string };
}) {
  const selectId = `e2e-select-${label.toLowerCase().replace(/\s+/g, '-')}`;
  return (
    <div className="selector-field">
      <label
        htmlFor={selectId}
        className="selector-label-row"
      >
        <span>{label}</span>
      </label>
      <Selector
        label={astryxLabel}
        isLabelHidden
        options={options}
        value={value}
        onChange={onChange}
        status={status}
      />
      <select
        id={selectId}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="e2e-select-proxy"
        tabIndex={-1}
      >
        {options.map((opt) => (
          <option key={opt.value} value={opt.value}>
            {opt.label}
          </option>
        ))}
      </select>
    </div>
  );
}
