import { expect, test } from "@playwright/test";
import { installDesktopBridge } from "./bridge";

test("projects resume their own state; opening the same video starts from the selected preset", async ({ page }) => {
	test.setTimeout(90000);
	await installDesktopBridge(page, "filmstrip.mp4");
	await page.addInitScript(() => {
		if (!localStorage.getItem("test-setup")) {
			localStorage.setItem("devcast.editor.presets", JSON.stringify([{ id: "studio", name: "Studio", createdAt: "2026-01-01", updatedAt: "2026-01-01", snapshot: { borderRadius: 5, borderRadiusUnit: "percent", cursorSize: 64 } }]));
			localStorage.setItem("devcast.editor.selectedPreset", JSON.stringify("studio"));
			localStorage.setItem("test-setup", "yes");
		}
		const source = `${location.origin}/tests/ui/fixtures/filmstrip.mp4`;
		const mode = localStorage.getItem("test-mode") || "project-a";
		Object.assign(window.electronAPI, {
			loadCurrentProjectFile: async () => mode === "video" ? { success: false } : { success: true, path: `/projects/${mode}.devcast`, project: { version: 2, videoPath: source, editor: { borderRadius: 1, cursorSize: 32, clipRegions: [{ id: "clip-1", startMs: 0, endMs: 6000, speed: 1 }] } } },
			saveProjectFile: async (_data: unknown, _name: string, target: string) => ({ success: true, path: target }),
			saveEditingSession: (project: unknown, projectPath: string) => { localStorage.setItem(`test-session:${projectPath}`, JSON.stringify(project)); return { success: true }; },
			loadEditingSession: async (_video: string, projectPath: string) => ({ success: true, project: JSON.parse(localStorage.getItem(`test-session:${projectPath}`) || "null") }),
		});
	});
	await page.goto("/?windowType=editor");
	const clips = page.locator('[data-variant="clip"]'), zooms = page.locator('[data-variant="zoom"]');
	const radius = page.getByRole("slider", { name: "Radius", exact: true });
	await expect(clips).toHaveAttribute("data-end-ms", "6000", { timeout: 20000 });
	await expect(radius).toHaveValue("1");
	await radius.focus(); await radius.press("Home");
	for (let i = 0; i < 17; i++) await radius.press("ArrowRight");
	await expect(radius).toHaveValue("1.7");
	await page.getByRole("button", { name: "Add Zoom (Z)", exact: true }).click();
	await expect(zooms).toHaveCount(1);
	await page.evaluate(() => localStorage.setItem("test-mode", "video")); await page.reload();
	await expect(radius).toHaveValue("5"); await expect(zooms).toHaveCount(0);
	await radius.focus(); await radius.press("Home"); await radius.press("ArrowRight");
	await page.evaluate(() => localStorage.setItem("test-mode", "project-a")); await page.reload();
	await expect(radius).toHaveValue("1.7"); await expect(zooms).toHaveCount(1);
	await page.evaluate(() => localStorage.setItem("test-mode", "project-b")); await page.reload();
	await expect(radius).toHaveValue("1"); await expect(zooms).toHaveCount(0);
	await radius.focus(); await radius.press("Home");
	for (let i = 0; i < 4; i++) await radius.press("ArrowRight");
	await expect(radius).toHaveValue("0.4");
	await page.evaluate(() => localStorage.setItem("test-mode", "project-a")); await page.reload();
	await expect(radius).toHaveValue("1.7"); await expect(zooms).toHaveCount(1);
	await page.evaluate(() => localStorage.setItem("test-mode", "project-b")); await page.reload();
	await expect(radius).toHaveValue("0.4"); await expect(zooms).toHaveCount(0);
});
