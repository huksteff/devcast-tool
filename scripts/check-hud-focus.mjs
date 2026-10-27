import { _electron as electron, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import path from 'node:path';
const root = process.cwd();
const server = spawn(process.execPath, ['node_modules/vite/bin/vite.js', 'preview', '--host', '127.0.0.1', '--port', '5190', '--strictPort'], { windowsHide: true, stdio: 'ignore' });
let app;
const errors = [];
try {
  await expect.poll(async () => { try { return (await fetch('http://127.0.0.1:5190')).status; } catch { return 0; } }).toBe(200);
  const packaged = process.argv.includes('--packaged');
  app = await electron.launch({ executablePath: path.join(root, packaged ? 'release/win-unpacked/DevCast.exe' : 'node_modules/electron/dist/electron.exe'), args: packaged ? [] : ['.'], env: { ...process.env, VITE_DEV_SERVER_URL: packaged ? '' : 'http://127.0.0.1:5190', DEVCAST_DEV_OPEN_RECORDING_INPUT: '' } });
  app.process().stderr?.on('data', chunk => process.stderr.write(chunk));
  await expect.poll(() => app.windows().find(w => w.url().includes('hud-overlay'))?.url()).toBeTruthy();
  const hud = app.windows().find(w => w.url().includes('hud-overlay'));
  hud.on('pageerror', e => errors.push(e.message));
  hud.on('console', message => { if (message.type() === 'error') errors.push(message.text()); });
  await app.evaluate(({ BrowserWindow }) => {
    const overlay = BrowserWindow.getAllWindows().find(w => w.webContents.getURL().includes('hud-overlay'));
    overlay.webContents.on('preload-error', (_event, preloadPath, error) => { overlay.__preloadError = { preloadPath, error: String(error) }; });
  });
  await expect.poll(() => hud.locator('button').count(), { timeout: 20000 }).toBeGreaterThan(2);
  await expect.poll(() => app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows().find(w => w.webContents.getURL().includes('hud-overlay'))?.isVisible())).toBe(true);
  await app.evaluate(async ({ BrowserWindow }) => {
    const target = new BrowserWindow({ width: 500, height: 300, title: 'HUD focus test target' });
    await target.loadURL('data:text/html,<title>HUD focus test target</title><p>Foreground test target</p>');
    target.focus();
  });
  const checkFocus = () => app.evaluate(({ BrowserWindow }) => {
    const hud = BrowserWindow.getAllWindows().find(w => w.webContents.getURL().includes('hud-overlay'));
    return { focusable: hud.isFocusable(), focused: hud.isFocused(), target: BrowserWindow.getFocusedWindow()?.getTitle() };
  });
  await expect.poll(checkFocus).toEqual({ focusable: false, focused: false, target: 'HUD focus test target' });
  // Exercise the real native-window lifecycle, including the shaped-input
  // Windows path that cannot be reproduced by browser-only UI tests.
  await app.evaluate(({ BrowserWindow }) => {
    const overlay = BrowserWindow.getAllWindows().find(w => w.webContents.getURL().includes('hud-overlay'));
    overlay.__inputRecovery = { ignores: [], shapes: 0 };
    const ignore = overlay.setIgnoreMouseEvents.bind(overlay);
    overlay.setIgnoreMouseEvents = (...args) => { overlay.__inputRecovery.ignores.push(args[0]); return ignore(...args); };
    const shape = overlay.setShape.bind(overlay);
    overlay.setShape = (...args) => { overlay.__inputRecovery.shapes++; return shape(...args); };
  });
  for (let cycle = 0; cycle < 3; cycle++) {
    await hud.evaluate(() => window.electronAPI.switchToEditor());
    await expect.poll(() => app.windows().filter(w => w.url().includes('windowType=editor')).length).toBe(1);
    await app.evaluate(({ BrowserWindow }) => {
      const editor = BrowserWindow.getAllWindows().find(w => w.webContents.getURL().includes('windowType=editor'));
      const overlay = BrowserWindow.getAllWindows().find(w => w.webContents.getURL().includes('hud-overlay'));
      overlay.__inputRecovery = { ignores: [], shapes: 0 };
      editor.close();
    });
    await expect.poll(() => app.evaluate(({ BrowserWindow }) => {
      const overlay = BrowserWindow.getAllWindows().find(w => w.webContents.getURL().includes('hud-overlay'));
      return { interactive: overlay.__inputRecovery.ignores.at(-1) === false, shaped: overlay.__inputRecovery.shapes > 0 };
    })).toEqual({ interactive: true, shaped: true });
  }
  const source = await hud.evaluate(() => window.electronAPI.getSelectedSource());
  await hud.getByRole('button', { name: source.name, exact: true }).click();
  await expect.poll(() => hud.locator('.source-selector-item').count(), { timeout: 15000 }).toBeGreaterThan(0);
  await expect.poll(checkFocus).toEqual({ focusable: false, focused: false, target: 'HUD focus test target' });
  await hud.locator('.source-selector-item').first().click();
  await expect.poll(() => hud.evaluate(() => window.electronAPI.getSelectedSource())).toHaveProperty('sourceType', 'screen');
  await expect.poll(checkFocus).toEqual({ focusable: false, focused: false, target: 'HUD focus test target' });
  expect(errors).toEqual([]);
  console.log('PASS: HUD input restored after 3 editor close cycles; non-activating; source menu opens and selects; foreground target preserved; no renderer errors');
} catch (error) {
  console.log('Renderer errors:', errors);
  if (app) console.log('Preload diagnostics:', await app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows().filter(w => w.webContents.getURL().includes('hud-overlay')).map(w => ({ error: w.__preloadError, preferences: w.webContents.getLastWebPreferences() }))));
  for (const page of app?.windows() ?? []) {
    console.log(page.url(), await page.locator('body').innerText().catch(() => 'closed'));
  }
  throw error;
} finally {
  if (app) {
    await app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows().filter(w => w.getTitle() === 'HUD focus test target').forEach(w => w.destroy()));
    await app.close();
  }
  server.kill();
}
