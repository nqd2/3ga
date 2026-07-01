import { expect, test } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import path from 'node:path';
import { serveDirectory } from './server';

async function routeMinimalSplat(page: import('@playwright/test').Page, repoRoot: string) {
  await page.route('**/tests/fixtures/minimal.splat', async (route) => {
    await route.fulfill({
      path: path.join(repoRoot, 'tests/fixtures/minimal.splat'),
      contentType: 'application/octet-stream',
    });
  });
}

async function pickTarget(page: import('@playwright/test').Page, target: string, x: number, y: number) {
  await page.getByLabel('Pick Target').selectOption(target);
  await page.locator('canvas.preview').click({ position: { x, y } });
}

test('GUI calibration flow serializes recipe and calls Tauri commands', async ({ page }) => {
  const repoRoot = path.resolve(process.cwd(), '../..');
  execFileSync('pnpm', ['run', 'build'], { cwd: process.cwd(), stdio: 'pipe' });

  const calls: Array<{ cmd: string; args: unknown }> = [];
  await page.addInitScript(() => {
    localStorage.setItem('ag_bake_geometry_profile', JSON.stringify('interior-room'));
    localStorage.removeItem('ag_bake_geometry_profile_version');
    (window as any).__TAURI_INTERNALS__ = {
      invoke: async (cmd: string, args: any) => {
        (window as any).__AG_CALLS__.push({ cmd, args });
        if (cmd === 'load_source') {
          return { path: args.path, bytes: 128, format: 'splat', splatCount: 4 };
        }
        if (cmd === 'export_edited_source') {
          return {
            path: 'target/gui-e2e-output/edited-source.ply',
            originalPath: args.request.inputPath,
            bytes: 256,
            splatCount: 4,
            bounds: { min: [0, 0, 0], max: [1, 1, 1] },
          };
        }
        if (cmd === 'process_job') {
          const request = args.request;
          const recipe = JSON.parse(request.recipeJson);
          return {
            version: 1,
            source: { format: 'splat', splatCount: 4, keptCount: 4 },
            artifacts: { manifest: 'manifest.json', collisionMeshJson: 'collision_mesh.json' },
            metrics: {
              collisionTrianglesAfterMerge: 12,
              serializedScaleDistance: recipe.alignmentRecipe.scaleDistanceMeters,
            },
          };
        }
        if (cmd === 'save_bundle') return args.request.destinationPath;
        if (cmd === 'cancel_job') return true;
        throw new Error(`unexpected command ${cmd}`);
      },
      transformCallback: () => 0,
      unregisterCallback: () => {},
      convertFileSrc: (filePath: string) => filePath,
    };
    (window as any).__AG_CALLS__ = [];
  });
  await routeMinimalSplat(page, repoRoot);

  const { server, url } = await serveDirectory(path.join(repoRoot, 'apps/gui/dist'));
  try {
    await page.goto(`${url}/index.html`);
    await page.getByLabel('Source PLY/SPLAT Path').fill('tests/fixtures/minimal.splat');
    await page.getByLabel('Output Directory').fill('target/gui-e2e-output');
    await page.getByRole('button', { name: 'Load' }).click();
    await expect(page.getByText('source loaded')).toBeVisible();
    await expect(page.getByText('Splats')).toBeVisible();

    await page.getByLabel('Scale Calibration Distance').fill('3.25');
    await pickTarget(page, 'scale0', 180, 220);
    await pickTarget(page, 'scale1', 320, 220);
    await page.getByLabel('Scale endpoints 1 x').fill('0.1');
    await page.getByLabel('Scale endpoints 2 x').fill('4');
    await page.getByLabel('Up Axis').selectOption('z');
    await page.getByRole('button', { name: 'Bake Geometry' }).click();
    await expect(page.getByText('done', { exact: true }).first()).toBeVisible();
    await expect(page.getByRole('cell', { name: '12' })).toBeVisible();

    await page.getByLabel('Output Directory').fill('target/gui-edited-output');
    await page.getByRole('button', { name: 'Save ZIP' }).click();
    await expect(page.getByText('saved target/gui-e2e-output/webar-copy.zip').first()).toBeVisible();

    calls.push(...(await page.evaluate(() => (window as any).__AG_CALLS__)));
  } finally {
    await new Promise<void>((resolve) => server.close(() => resolve()));
  }

  const processCall = calls.find((call) => call.cmd === 'process_job');
  expect(processCall).toBeTruthy();
  const request = (processCall!.args as any).request;
  const recipe = JSON.parse(request.recipeJson);
  expect(recipe.alignmentRecipe.scaleDistanceMeters).toBe(3.25);
  expect(recipe.alignmentRecipe.scalePoints[1][0]).toBe(4);
  expect(recipe.alignmentRecipe.upAxis).toBe('z');
  expect(recipe.alignmentRecipe.floorPoints).toBeUndefined();
  expect(recipe.alignmentRecipe.floorFitPoints).toBeUndefined();
  expect(recipe.alignmentRecipe.floorNormal).toEqual([0, 0, 1]);
  expect(recipe.alignmentRecipe.origin).toEqual([0, 0, 0]);
  const config = JSON.parse(request.configJson);
  expect(config.voxel.backend).toBe('cpu');
  expect(config.voxel.size).toBe(0.05);
  expect(config.voxelFill.mode).toBe('none');
  expect(config.voxelCarve.enabled).toBe(false);
  expect(config.navmesh.enabled).toBe(false);
  expect(config.mesh.mode).toBe('smooth');
  expect(request.inputPath).toBe('target/gui-e2e-output/edited-source.ply');
  expect(request.sourceContext).toMatchObject({
    originalPath: 'tests/fixtures/minimal.splat',
    editedPath: 'target/gui-e2e-output/edited-source.ply',
    editedSplatCount: 4,
    editedBytes: 256,
  });
  expect(recipe.editRecipe.operations).toEqual([]);
  const saveCall = calls.find((call) => call.cmd === 'save_bundle');
  expect((saveCall!.args as any).request).toEqual({
    outDir: 'target/gui-e2e-output',
    destinationPath: 'target/gui-e2e-output/webar-copy.zip',
  });
  const callNames = calls.map((call) => call.cmd);
  expect(callNames).toContain('plugin:event|listen');
  expect(callNames.filter((cmd) => cmd !== 'plugin:event|listen')).toEqual([
    'load_source',
    'export_edited_source',
    'process_job',
    'save_bundle',
  ]);
});

test('progress events keep job controls locked while cancel stays available', async ({ page }) => {
  const repoRoot = path.resolve(process.cwd(), '../..');
  execFileSync('pnpm', ['run', 'build'], { cwd: process.cwd(), stdio: 'pipe' });

  await page.addInitScript(() => {
    (window as any).__AG_CALLBACKS__ = [];
    (window as any).__AG_RESOLVE_PROCESS__ = null;
    (window as any).__TAURI_INTERNALS__ = {
      invoke: async (cmd: string, args: any) => {
        if (cmd === 'plugin:event|listen') return 1;
        if (cmd === 'plugin:event|unlisten') return true;
        if (cmd === 'load_source') {
          return { path: args.path, bytes: 128, format: 'splat', splatCount: 4 };
        }
        if (cmd === 'export_edited_source') {
          return {
            path: 'target/gui-e2e-output/edited-source.ply',
            originalPath: args.request.inputPath,
            bytes: 256,
            splatCount: 4,
            bounds: { min: [0, 0, 0], max: [1, 1, 1] },
          };
        }
        if (cmd === 'process_job') {
          (window as any).__AG_PROCESS_REQUEST__ = args.request;
          return new Promise((resolve) => {
            (window as any).__AG_RESOLVE_PROCESS__ = () =>
              resolve({
                version: 1,
                source: { format: 'splat', splatCount: 4, keptCount: 4 },
                artifacts: { manifest: 'manifest.json', collisionMeshJson: 'collision_mesh.json' },
                metrics: { collisionTrianglesAfterMerge: 12 },
              });
          });
        }
        if (cmd === 'cancel_job') return true;
        throw new Error(`unexpected command ${cmd}`);
      },
      transformCallback: (callback: unknown) => {
        (window as any).__AG_CALLBACKS__.push(callback);
        return (window as any).__AG_CALLBACKS__.length - 1;
      },
      unregisterCallback: () => {},
      convertFileSrc: (filePath: string) => filePath,
    };
  });
  await routeMinimalSplat(page, repoRoot);

  const { server, url } = await serveDirectory(path.join(repoRoot, 'apps/gui/dist'));
  try {
    await page.goto(`${url}/index.html`);
    await page.waitForFunction(() => (window as any).__AG_CALLBACKS__.length > 0);
    await page.getByLabel('Source PLY/SPLAT Path').fill('tests/fixtures/minimal.splat');
    await page.getByRole('button', { name: 'Load' }).click();
    await expect(page.getByText('source loaded')).toBeVisible();

    await pickTarget(page, 'scale0', 180, 220);
    await pickTarget(page, 'scale1', 320, 220);
    await page.getByLabel('Scale endpoints 1 x').fill('0.1');
    await page.getByLabel('Scale endpoints 2 x').fill('2.1');

    await page.getByRole('button', { name: 'Bake Geometry' }).click();
    await page.waitForFunction(() => Boolean((window as any).__AG_RESOLVE_PROCESS__));
    await page.evaluate(() => {
      (window as any).__AG_CALLBACKS__[0]({ payload: { stage: 'voxelize' } });
    });
    await expect(page.getByText('processing: voxelize', { exact: true }).first()).toBeVisible();
    await expect(page.getByRole('button', { name: 'Load' })).toBeDisabled();
    await expect(page.getByRole('button', { name: 'Bake Geometry' })).toBeDisabled();
    await expect(page.getByRole('button', { name: 'Cancel' })).toBeEnabled();

    await page.getByRole('button', { name: 'Cancel' }).click();
    await expect(page.getByText('cancel requested').first()).toBeVisible();
    await expect(page.getByRole('button', { name: 'Cancel' })).toBeEnabled();

    await page.evaluate(() => (window as any).__AG_RESOLVE_PROCESS__());
    await expect(page.getByText('done', { exact: true }).first()).toBeVisible();
    await expect(page.getByRole('button', { name: 'Cancel' })).toBeDisabled();

    const recipe = await page.evaluate(() => JSON.parse((window as any).__AG_PROCESS_REQUEST__.recipeJson));
    expect(recipe.alignmentRecipe.origin).toEqual([0, 0, 0]);
    expect(recipe.alignmentRecipe.floorNormal).toEqual([0, 1, 0]);
  } finally {
    await new Promise<void>((resolve) => server.close(() => resolve()));
  }
});

test('up axis mode serializes alignment without floor point recipes', async ({ page }) => {
  const repoRoot = path.resolve(process.cwd(), '../..');
  execFileSync('pnpm', ['run', 'build'], { cwd: process.cwd(), stdio: 'pipe' });

  await page.addInitScript(() => {
    (window as any).__AG_PROCESS_REQUEST__ = null;
    (window as any).__TAURI_INTERNALS__ = {
      invoke: async (cmd: string, args: any) => {
        if (cmd === 'plugin:event|listen') return 1;
        if (cmd === 'plugin:event|unlisten') return true;
        if (cmd === 'load_source') {
          return { path: args.path, bytes: 128, format: 'splat', splatCount: 4 };
        }
        if (cmd === 'export_edited_source') {
          return {
            path: 'target/gui-e2e-output/edited-source.ply',
            originalPath: args.request.inputPath,
            bytes: 256,
            splatCount: 4,
            bounds: { min: [0, 0, 0], max: [1, 1, 1] },
          };
        }
        if (cmd === 'process_job') {
          (window as any).__AG_PROCESS_REQUEST__ = args.request;
          return {
            version: 1,
            source: { format: 'splat', splatCount: 4, keptCount: 4 },
            artifacts: { manifest: 'manifest.json', collisionMeshJson: 'collision_mesh.json' },
            metrics: { collisionTrianglesAfterMerge: 12 },
          };
        }
        throw new Error(`unexpected command ${cmd}`);
      },
      transformCallback: () => 0,
      unregisterCallback: () => {},
      convertFileSrc: (filePath: string) => filePath,
    };
  });
  await routeMinimalSplat(page, repoRoot);

  const { server, url } = await serveDirectory(path.join(repoRoot, 'apps/gui/dist'));
  try {
    await page.goto(`${url}/index.html`);
    await page.getByLabel('Source PLY/SPLAT Path').fill('tests/fixtures/minimal.splat');
    await page.getByRole('button', { name: 'Load' }).click();
    await expect(page.getByText('source loaded')).toBeVisible();

    await pickTarget(page, 'scale0', 180, 220);
    await pickTarget(page, 'scale1', 320, 220);
    await page.getByLabel('Scale endpoints 1 x').fill('0.1');
    await page.getByLabel('Scale endpoints 2 x').fill('2.1');
    await page.getByLabel('Up Axis').selectOption('z');

    await page.getByRole('button', { name: 'Bake Geometry' }).click();
    await expect(page.getByText('done', { exact: true }).first()).toBeVisible();

    const recipe = await page.evaluate(() => JSON.parse((window as any).__AG_PROCESS_REQUEST__.recipeJson));
    expect(recipe.alignmentRecipe.floorFitPoints).toBeUndefined();
    expect(recipe.alignmentRecipe.floorPoints).toBeUndefined();
    expect(recipe.alignmentRecipe.floorNormal).toEqual([0, 0, 1]);
    expect(recipe.alignmentRecipe.origin).toEqual([0, 0, 0]);
  } finally {
    await new Promise<void>((resolve) => server.close(() => resolve()));
  }
});

test('source loading keeps controls responsive and can be cancelled', async ({ page }) => {
  const repoRoot = path.resolve(process.cwd(), '../..');
  execFileSync('pnpm', ['run', 'build'], { cwd: process.cwd(), stdio: 'pipe' });

  let releaseSplat!: () => void;
  const splatRelease = new Promise<void>((resolve) => {
    releaseSplat = resolve;
  });
  let markRouteStarted!: () => void;
  const routeStarted = new Promise<void>((resolve) => {
    markRouteStarted = resolve;
  });
  await page.route('**/tests/fixtures/minimal.splat', async (route) => {
    markRouteStarted();
    await splatRelease;
    try {
      await route.fulfill({
        path: path.join(repoRoot, 'tests/fixtures/minimal.splat'),
        contentType: 'application/octet-stream',
      });
    } catch {
      // The app aborts the fetch when Cancel is clicked; the route may already be closed.
    }
  });

  await page.addInitScript(() => {
    (window as any).__TAURI_INTERNALS__ = {
      invoke: async (cmd: string, args: any) => {
        if (cmd === 'plugin:event|listen') return 1;
        if (cmd === 'plugin:event|unlisten') return true;
        if (cmd === 'load_source') {
          return { path: args.path, bytes: 128, format: 'splat', splatCount: 4 };
        }
        throw new Error(`unexpected command ${cmd}`);
      },
      transformCallback: () => 0,
      unregisterCallback: () => {},
      convertFileSrc: (filePath: string) => filePath,
    };
  });

  const { server, url } = await serveDirectory(path.join(repoRoot, 'apps/gui/dist'));
  try {
    await page.goto(`${url}/index.html`);
    await page.getByLabel('Source PLY/SPLAT Path').fill('tests/fixtures/minimal.splat');
    await page.getByRole('button', { name: 'Load' }).click();
    await routeStarted;

    await expect(page.getByText('loading source', { exact: true }).first()).toBeVisible();
    await expect(page.getByRole('button', { name: 'Load' })).toBeDisabled();
    await expect(page.getByRole('button', { name: 'Bake Geometry' })).toBeDisabled();
    await expect(page.getByRole('button', { name: 'Cancel' })).toBeEnabled();

    await page.getByRole('button', { name: 'Cancel' }).click();
    await expect(page.getByText('source load cancelled').first()).toBeVisible();
    await expect(page.getByRole('button', { name: 'Cancel' })).toBeDisabled();
  } finally {
    releaseSplat();
    await new Promise<void>((resolve) => server.close(() => resolve()));
  }
});
