import { expect, test } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import path from 'node:path';
import { serveDirectory } from './server';

test('GUI calibration flow serializes recipe and calls Tauri commands', async ({ page }) => {
  const repoRoot = path.resolve(process.cwd(), '../..');
  execFileSync('pnpm', ['run', 'build'], { cwd: process.cwd(), stdio: 'pipe' });

  const calls: Array<{ cmd: string; args: unknown }> = [];
  await page.addInitScript(() => {
    (window as any).__TAURI_INTERNALS__ = {
      invoke: async (cmd: string, args: any) => {
        (window as any).__AG_CALLS__.push({ cmd, args });
        if (cmd === 'load_source') {
          return { path: args.path, bytes: 128, format: 'splat', splatCount: 4 };
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
    };
    (window as any).__AG_CALLS__ = [];
  });

  const { server, url } = await serveDirectory(path.join(repoRoot, 'apps/gui/dist'));
  try {
    await page.goto(`${url}/index.html`);
    await page.getByLabel('Source path').fill('tests/fixtures/minimal.splat');
    await page.getByLabel('Output dir').fill('target/gui-e2e-output');
    await page.getByRole('button', { name: 'Load' }).click();
    await expect(page.getByText('source loaded')).toBeVisible();
    await expect(page.getByText('Rows')).toBeVisible();

    await page.getByLabel('Scale calibration distance (m)').fill('3.25');
    await page.getByLabel('Floor mode').selectOption('fit');
    await page.getByLabel('Floor fit points 4 y').fill('0.04');
    await page.getByLabel('Scale endpoints 2 x').fill('4');
    await page.getByLabel('Up axis').selectOption('z');
    await page.getByRole('button', { name: 'Bake' }).click();
    await expect(page.getByText('done')).toBeVisible();
    await expect(page.getByText('12')).toBeVisible();

    await page.getByRole('button', { name: 'Save ZIP' }).click();
    await expect(page.getByText('saved target/gui-e2e-output/webar-copy.zip')).toBeVisible();

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
  expect(recipe.alignmentRecipe.floorFitPoints).toHaveLength(4);
  expect(recipe.alignmentRecipe.floorFitPoints[3][1]).toBe(0.04);
  expect(JSON.parse(request.configJson).voxel.backend).toBe('cpu');
  expect(JSON.parse(request.configJson).mesh.mode).toBe('smooth');
  const callNames = calls.map((call) => call.cmd);
  expect(callNames).toContain('plugin:event|listen');
  expect(callNames.filter((cmd) => cmd !== 'plugin:event|listen')).toEqual([
    'load_source',
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
        if (cmd === 'process_job') {
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

  const { server, url } = await serveDirectory(path.join(repoRoot, 'apps/gui/dist'));
  try {
    await page.goto(`${url}/index.html`);
    await page.waitForFunction(() => (window as any).__AG_CALLBACKS__.length > 0);
    await page.getByLabel('Source path').fill('tests/fixtures/minimal.splat');
    await page.getByRole('button', { name: 'Load' }).click();
    await expect(page.getByText('source loaded')).toBeVisible();

    await page.getByRole('button', { name: 'Bake' }).click();
    await page.waitForFunction(() => Boolean((window as any).__AG_RESOLVE_PROCESS__));
    await page.evaluate(() => {
      (window as any).__AG_CALLBACKS__[0]({ payload: { stage: 'voxelize' } });
    });
    await expect(page.getByText('processing: voxelize')).toBeVisible();
    await expect(page.getByRole('button', { name: 'Load' })).toBeDisabled();
    await expect(page.getByRole('button', { name: 'Bake' })).toBeDisabled();
    await expect(page.getByRole('button', { name: 'Cancel' })).toBeEnabled();

    await page.getByRole('button', { name: 'Cancel' }).click();
    await expect(page.getByText('cancel requested')).toBeVisible();
    await expect(page.getByRole('button', { name: 'Cancel' })).toBeEnabled();

    await page.evaluate(() => (window as any).__AG_RESOLVE_PROCESS__());
    await expect(page.getByText('done')).toBeVisible();
    await expect(page.getByRole('button', { name: 'Cancel' })).toBeDisabled();
  } finally {
    await new Promise<void>((resolve) => server.close(() => resolve()));
  }
});
