import React, { useMemo } from 'react';
import { Card } from '@astryxdesign/core/Card';
import { Badge } from '@astryxdesign/core/Badge';
import { Text } from '@astryxdesign/core/Text';
import { ProgressBar } from '@astryxdesign/core/ProgressBar';
import { Table, proportional } from '@astryxdesign/core/Table';
import { Button } from '@astryxdesign/core/Button';
import { getStageLabel, type Manifest } from '../utils/jobHelpers';
import { type ViewerLoadProgress } from '../classes/PlayCanvasViewer';
import { formatBytes, formatNumber, formatRatio, formatBounds } from '../utils/formatters';
import { type SourceMetadata } from './Preview';

export function StatusPanel({
  status,
  progressStage,
  progressPct,
  isSourceLoading,
  sourceLoadProgress,
  isProcessing,
  logs,
  logEndRef,
  sourceMetadata,
  manifest,
  saveBundle,
  previewWebAr,
}: {
  status: string;
  progressStage: string;
  progressPct: number;
  isSourceLoading: boolean;
  sourceLoadProgress: ViewerLoadProgress | null;
  isProcessing: boolean;
  logs: string[];
  logEndRef: React.RefObject<HTMLDivElement | null>;
  sourceMetadata: SourceMetadata | null;
  manifest: Manifest | null;
  saveBundle: () => void;
  previewWebAr: () => void;
}) {
  const sourceTableData: Record<string, unknown>[] = useMemo(() => {
    if (!sourceMetadata) return [];
    return [
      { metric: 'Format', value: sourceMetadata.format.toUpperCase() },
      { metric: 'Splats', value: sourceMetadata.splatCount.toLocaleString() },
      { metric: 'File Size', value: formatBytes(sourceMetadata.bytes) },
      { metric: 'Bounds', value: formatBounds(sourceMetadata.bounds) },
    ];
  }, [sourceMetadata]);

  const manifestTableData: Record<string, unknown>[] = useMemo(() => {
    if (!manifest) return [];
    const metrics = manifest.metrics as Record<string, any>;
    return [
      { metric: 'Output Format', value: manifest.source.format },
      ...(manifest.source.originalPath ? [{ metric: 'Original Source', value: manifest.source.originalPath }] : []),
      ...(manifest.source.editedPath ? [{ metric: 'Edited Source', value: manifest.source.editedPath }] : []),
      ...(manifest.source.editedSplatCount ? [{ metric: 'Edited Splats', value: manifest.source.editedSplatCount.toLocaleString() }] : []),
      ...(manifest.source.editedBytes ? [{ metric: 'Edited Bytes', value: formatBytes(manifest.source.editedBytes) }] : []),
      { metric: 'Kept Splats', value: manifest.source.keptCount.toLocaleString() },
      { metric: 'Cluster Removed', value: formatNumber(metrics.filterClusterRemovedCount ?? 0) },
      { metric: 'Cluster Cells', value: formatNumber(metrics.filterClusterCells ?? 0) },
      { metric: 'Fill Cells', value: formatNumber(metrics.filledSolidCells ?? 0) },
      { metric: 'Carve Cells', value: formatNumber(metrics.carvedSolidCells ?? 0) },
      { metric: 'Cropped Cells', value: formatNumber(metrics.croppedSolidCells ?? 0) },
      { metric: 'Bounds', value: formatBounds(manifest.bounds) },
      { metric: 'Collision Triangles', value: formatNumber(metrics.collisionTrianglesAfterMerge) },
      { metric: 'Navmesh Triangles', value: formatNumber(metrics.navmeshTriangles) },
      { metric: 'Source Size', value: formatBytes(metrics.sourceBytes) },
      { metric: 'Optimized GLB', value: formatBytes(metrics.optimizedGlbBytes) },
      { metric: 'Size Ratio', value: formatRatio(metrics.sourceToOptimizedGlbRatio) },
    ];
  }, [manifest]);

  const collisionWarnings = useMemo(() => {
    const warnings = manifest?.metrics?.collisionWarnings;
    return Array.isArray(warnings) ? warnings.filter((value): value is string => typeof value === 'string') : [];
  }, [manifest]);

  const summaryTableColumns = useMemo(() => [
    { key: 'metric', header: 'Metric', width: proportional(1) },
    { key: 'value', header: 'Value', width: proportional(1.5) }
  ], []);

  return (
    <aside className="panel">
      <Card variant="default" padding={4}>
        <div className="card-stack">
          <div>
            <div className="section-title-row">
              <h3 className="config-group-title">Job Status</h3>
              <Badge
                variant={status.startsWith('error') ? 'error' : status === 'done' ? 'success' : 'info'}
                label={getStageLabel(progressStage)}
              />
            </div>
            <div className="progress-meta">
              <Text type="label" color="primary" as="span">{status}</Text>
              <Text type="label" color="secondary" as="span">{progressPct}%</Text>
            </div>
            <ProgressBar
              label="Pipeline progress"
              value={progressPct}
              hasValueLabel={false}
              isLabelHidden
              variant={status.startsWith('error') ? 'error' : status === 'done' ? 'success' : 'accent'}
              isIndeterminate={(isSourceLoading && sourceLoadProgress?.percent == null) || (isProcessing && progressPct === 0)}
              isDisabled={!isProcessing && !isSourceLoading && status !== 'done'}
            />
            <div className="stage-strip" aria-label="Pipeline stages">
              {['decode', 'align', 'filter', 'voxelize', 'fill', 'carve', 'mesh', 'navmesh', 'export'].map((stage) => (
                <span
                  key={stage}
                  className="stage-step"
                  data-active={getStageLabel(progressStage) === stage ? 'true' : undefined}
                >
                  {stage}
                </span>
              ))}
            </div>
          </div>

          <div>
            <h3 className="config-group-title section-title">Console Feed</h3>
            <div className="log-console">
              {logs.length === 0 ? (
                <div className="log-line log-line-muted">System ready. Load a source model to begin.</div>
              ) : (
                logs.map((log, idx) => (
                  <div key={idx} className="log-line">{log}</div>
                ))
              )}
              <div ref={logEndRef} />
            </div>
          </div>

          {sourceMetadata && (
            <div>
              <h3 className="config-group-title section-title">Source Metadata</h3>
              <Table
                data={sourceTableData}
                columns={summaryTableColumns}
                idKey="metric"
                density="compact"
              />
            </div>
          )}

          {manifest && (
            <div className="card-stack card-stack-tight">
              <h3 className="config-group-title">Manifest Details</h3>
              <Table
                data={manifestTableData}
                columns={summaryTableColumns}
                idKey="metric"
                density="compact"
              />
              {collisionWarnings.length > 0 && (
                <div className="manifest-warnings" aria-label="Collision warnings">
                  {collisionWarnings.map((warning) => (
                    <div key={warning} className="manifest-warning">{warning}</div>
                  ))}
                </div>
              )}
              <div className="manifest-actions">
                <Button
                  label="Save ZIP"
                  variant="primary"
                  isDisabled={isProcessing}
                  onClick={saveBundle}
                  className="manifest-action-button"
                />
                <Button
                  label="Preview WebAR"
                  variant="secondary"
                  isDisabled={isProcessing}
                  onClick={previewWebAr}
                  className="manifest-action-button"
                />
              </div>
            </div>
          )}
        </div>
      </Card>
    </aside>
  );
}
