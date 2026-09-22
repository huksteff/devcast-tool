import { _electron as electron, expect } from '@playwright/test';
import { spawn, execFileSync } from 'node:child_process';
import path from 'node:path';
import fs from 'node:fs';

const root = process.cwd();
const out = path.join(root, '.tmp', 'timeline-check');
fs.mkdirSync(out, { recursive: true });
const media = path.join(out, 'sample.mp4');
execFileSync(path.join(root, 'node_modules/ffmpeg-static/ffmpeg.exe'), ['-y', '-f', 'lavfi', '-i', 'testsrc2=size=640x360:rate=25', '-t', '6', '-c:v', 'libx264', media], { stdio: 'ignore' });
const server = spawn(process.execPath, ['node_modules/vite/bin/vite.js', 'preview', '--host', '127.0.0.1', '--port', '5189', '--strictPort'], { cwd: root, stdio: 'ignore', windowsHide: true });
let app;
const errors = [];
try {
  await expect.poll(async () => { try { return (await fetch('http://127.0.0.1:5189')).status; } catch { return 0; } }).toBe(200);
  const packaged = process.argv.includes('--packaged');
  app = await electron.launch({ executablePath: path.join(root, packaged ? 'release/win-unpacked/Recordly.exe' : 'node_modules/electron/dist/electron.exe'), args: packaged ? [] : ['.'], env: { ...process.env, VITE_DEV_SERVER_URL: 'http://127.0.0.1:5189', RECORDLY_DEV_OPEN_RECORDING_INPUT: media }, timeout: 30000 });
  app.on('window', page => page.on('pageerror', error => errors.push(error.message)));
  await expect.poll(() => app.windows().find(p => p.url().includes('windowType=editor'))?.url(), { timeout: 30000 }).toBeTruthy();
  const editor = app.windows().find(p => p.url().includes('windowType=editor'));
  await editor.getByRole('button', { name: 'Open Timeline Window' }).waitFor({ timeout: 30000 });
  const source = await editor.evaluate(() => window.electronAPI.getSelectedSource());
  const primary = await app.evaluate(({ screen }) => String(screen.getPrimaryDisplay().id));
  expect(source.sourceType).toBe('screen');
  expect(source.display_id).toBe(primary);
  await editor.getByRole('button', { name: 'Open Timeline Window' }).click();
  await expect.poll(() => app.windows().find(p => p.url().includes('windowType=timeline-window'))?.url()).toBeTruthy();
  const child = app.windows().find(p => p.url().includes('windowType=timeline-window'));
  await child.getByTestId('timeline-scroll').waitFor({ timeout: 15000 });
  // Packaged child windows load from file, while the editor uses the local server.
  await app.evaluate(async ({ BrowserWindow, app }) => {
    const child = BrowserWindow.getAllWindows().find(w => w.webContents.getURL().includes('windowType=timeline-window'));
    await child.loadFile(app.getAppPath() + '/dist/index.html', { query: { windowType: 'timeline-window' } });
  });
  await child.getByTestId('timeline-scroll').waitFor({ timeout: 15000 });
  expect(await child.getByTestId('timeline-scroll').evaluate(e => e.getBoundingClientRect().height)).toBeGreaterThan(100);
  await expect(editor.getByTestId('timeline-scroll')).toBeHidden();
  await child.evaluate(() => window.electronAPI.sendTimelineWindowMessage({ type: 'action', action: { type: 'seek', time: 2 } }));
  await expect.poll(() => editor.evaluate(() => document.querySelector('video')?.currentTime), { timeout: 10000 }).toBeGreaterThan(1.8);
  await child.evaluate(() => window.electronAPI.sendTimelineWindowMessage({ type: 'action', action: { type: 'zoom-added', span: { start: 1000, end: 3000 } } }));
  await expect.poll(() => child.locator('[data-timeline-item]').count()).toBeGreaterThan(1);
  await expect.poll(() => child.locator('[data-testid="clip-filmstrip"] img').count(), { timeout: 20000 }).toBeGreaterThan(0);
  await child.screenshot({ path: path.join(out, 'detached.png') });
  await child.getByRole('button', { name: 'Close window' }).click();
  await expect(editor.getByTestId('timeline-scroll')).toBeVisible();
  expect(errors).toEqual([]);
  console.log('PASS: primary monitor; detached render and height; dock hidden; seek; edit; redock; no renderer errors');
} finally {
  console.log('Renderer errors:', errors);
  await app?.close();
  server.kill();
}
