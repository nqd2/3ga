import React, { useCallback, useMemo } from 'react';
import { createRoot } from 'react-dom/client';

import { Theme } from '@astryxdesign/core/theme';
import { Button } from '@astryxdesign/core/Button';
import '@astryxdesign/core/reset.css';
import '@astryxdesign/core/astryx.css';
import { stoneTheme } from '@astryxdesign/theme-stone';
import '@astryxdesign/theme-stone/theme.css';

import { Header } from './components/Header';
import { ConfigPanel } from './components/ConfigPanel';
import { Preview } from './components/Preview';
import { StatusPanel } from './components/StatusPanel';
import { EditorPanel } from './domains/editor/EditorPanel';

import { useActivityLog } from './hooks/useActivityLog';
import { useSourceLoader } from './hooks/useSourceLoader';
import { useCalibration } from './hooks/useCalibration';
import { useBakeJob } from './hooks/useBakeJob';
import { useEditorScene } from './domains/editor/useEditorScene';
import { stageProgress } from './utils/jobHelpers';
import './styles/styles.css';

function App() {
  const activityLog = useActivityLog();
  const sourceLoader = useSourceLoader({ setStatus: activityLog.setStatus });
  const calibration = useCalibration();
  const editor = useEditorScene({
    sourcePath: sourceLoader.sourceMetadata?.path ?? null,
    splatCount: sourceLoader.sourceMetadata?.splatCount ?? 0,
    bounds: sourceLoader.sourceMetadata?.bounds ?? null,
  });

  const isCalibrationValid = useMemo(() =>
    sourceLoader.isSourceReady && calibration.distance > 0 &&
    calibration.isFloorCalibrated && calibration.isScaleCalibrated &&
    editor.visible && !editor.deleted,
    [
      sourceLoader.isSourceReady,
      calibration.distance,
      calibration.isFloorCalibrated,
      calibration.isScaleCalibrated,
      editor.visible,
      editor.deleted,
    ]
  );

  const prepareBakeSource = useCallback(async () => {
    activityLog.setStatus('exporting edited source');
    const exported = await editor.exportVisibleMergedSource();
    return {
      inputPath: exported.path,
      sourceContext: {
        originalPath: exported.originalPath,
        editedPath: exported.path,
        editedSplatCount: exported.splatCount,
        editedBytes: exported.bytes,
      },
    };
  }, [activityLog, editor]);

  const bakeJob = useBakeJob({
    status: activityLog.status,
    setStatus: activityLog.setStatus,
    inputPath: sourceLoader.inputPath,
    recipe: calibration.recipe,
    config: calibration.processConfig,
    isCalibrationValid,
    prepareBakeSource,
  });

  const isBusy = bakeJob.isProcessing || sourceLoader.isSourceLoading;

  const progressPct = useMemo(() => {
    if (sourceLoader.isSourceLoading) {
      return Math.round(sourceLoader.sourceLoadProgress?.percent ?? 0);
    }
    if (activityLog.status.startsWith('error')) {
      return 100;
    }
    return stageProgress[bakeJob.progressStage] || 0;
  }, [sourceLoader.isSourceLoading, sourceLoader.sourceLoadProgress, activityLog.status, bakeJob.progressStage]);

  const cancelJob = useCallback(async () => {
    if (sourceLoader.isSourceLoading) {
      sourceLoader.cancelLoad();
    } else {
      await bakeJob.cancelJob();
    }
  }, [sourceLoader, bakeJob]);

  const loadSource = useCallback(() => {
    editor.resetScene();
    sourceLoader.loadSource();
  }, [editor, sourceLoader]);

  return (
    <Theme theme={stoneTheme} mode="dark">
      <main className="app-shell astryx-theme">
        <Header
          inputPath={sourceLoader.inputPath}
          sourceMetadata={sourceLoader.sourceMetadata}
          isBusy={isBusy}
          isCalibrationValid={isCalibrationValid}
          loadSource={loadSource}
          runProcess={bakeJob.runProcess}
          cancelJob={cancelJob}
        />

        <nav className="view-tabs" aria-label="Application views">
          <Button
            label="Editor"
            variant={editor.activeView === 'editor' ? 'primary' : 'secondary'}
            onClick={() => editor.setActiveView('editor')}
          />
          <Button
            label="AR Bake"
            variant={editor.activeView === 'bake' ? 'primary' : 'secondary'}
            onClick={() => editor.setActiveView('bake')}
          />
        </nav>

        <section className="workspace">
          {editor.activeView === 'editor' ? (
            <EditorPanel
              inputPath={sourceLoader.inputPath}
              setInputPath={sourceLoader.setInputPath}
              handleBrowseInputPath={sourceLoader.handleBrowseInputPath}
              scene={editor.sceneState}
              cameraMode={editor.cameraMode}
              setCameraMode={editor.setCameraMode}
              setSceneVisible={editor.setVisible}
              deleteScene={() => editor.setDeleted(true)}
              resetScene={editor.resetScene}
              setTransformAxis={editor.setTransformAxis}
              resetTransform={editor.resetTransform}
              isBusy={isBusy}
            />
          ) : (
            <ConfigPanel
              inputPath={sourceLoader.inputPath}
              setInputPath={sourceLoader.setInputPath}
              handleBrowseInputPath={sourceLoader.handleBrowseInputPath}
              outDir={bakeJob.outDir}
              setOutDir={bakeJob.setOutDir}
              handleBrowseOutDir={bakeJob.handleBrowseOutDir}
              distance={calibration.distance}
              setDistance={calibration.setDistance}
              pickMode={calibration.pickMode}
              setPickMode={calibration.setPickMode}
              upAxis={calibration.upAxis}
              setUpAxis={calibration.setUpAxis}
              geometryProfile={calibration.geometryProfile}
              setGeometryProfile={calibration.setGeometryProfile}
              sceneTransform={editor.transform}
              setSceneTransformAxis={editor.setTransformAxis}
              resetSceneTransform={editor.resetTransform}
              scalePoints={calibration.scalePoints}
              updateScalePoint={calibration.updateScalePoint}
              userPickedScalePoints={calibration.userPickedScalePoints}
              isProcessing={bakeJob.isProcessing}
            />
          )}

          <section className="viewport-container">
            <Preview
              sourceUrl={sourceLoader.sourceUrl}
              sourceMetadata={sourceLoader.sourceMetadata}
              bounds={sourceLoader.sourceMetadata?.bounds}
              scalePoints={calibration.scalePoints}
              onPick={calibration.handlePick}
              pickMode={calibration.pickMode}
              onSourceProgress={sourceLoader.handleSourceProgress}
              onSourceReady={sourceLoader.handleSourceReady}
              onSourceError={sourceLoader.handleSourceError}
              upAxis={calibration.upAxis}
              sceneTransform={editor.transform}
              cameraMode={editor.cameraMode}
              sceneVisible={editor.visible && !editor.deleted}
            />
          </section>

          <StatusPanel
            status={activityLog.status}
            progressStage={bakeJob.progressStage}
            progressPct={progressPct}
            isSourceLoading={sourceLoader.isSourceLoading}
            sourceLoadProgress={sourceLoader.sourceLoadProgress}
            isProcessing={bakeJob.isProcessing}
            logs={activityLog.logs}
            logEndRef={activityLog.logEndRef}
            sourceMetadata={sourceLoader.sourceMetadata}
            manifest={bakeJob.manifest}
            saveBundle={bakeJob.saveBundle}
            previewWebAr={bakeJob.previewWebAr}
          />
        </section>
      </main>
    </Theme>
  );
}

const container = document.getElementById('root');
if (container) {
  createRoot(container).render(<App />);
}
