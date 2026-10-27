import { expect, test } from "@playwright/test";
import { installDesktopBridge } from "./bridge";

test("manual zoom area resizes continuously and retains its size after deselection", async ({ page }) => {
	await installDesktopBridge(page, "filmstrip.mp4");
	await page.goto("/?windowType=editor");
	await expect(page.getByLabel("Loading preview")).toHaveCount(0);
	await expect(page.locator('[data-variant="clip"]')).toHaveAttribute("data-end-ms", "6000", { timeout: 20000 });
	await page.getByRole("button", { name: "Add Zoom (Z)", exact: true }).click();
	await page.locator('[data-variant="zoom"]').click();
	await page.getByRole("row", { name: "Manual", exact: true }).click();
	const area = page.getByTestId("zoom-focus-area");
	await expect(area).toBeVisible();
	const before = (await area.boundingBox())!;
	const handle = page.getByRole("slider", { name: "Resize zoom se", exact: true });
	const box = (await handle.boundingBox())!;
	await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
	await page.mouse.down();
	await page.mouse.move(box.x - 90, box.y - 50, { steps: 10 });
	await page.mouse.up();
	await expect.poll(async () => (await area.boundingBox())!.width).toBeLessThan(before.width - 30);
	const after = (await area.boundingBox())!;
	expect(after.x).toBeCloseTo(before.x, 0);
	expect(after.y).toBeCloseTo(before.y, 0);
	await page.locator('[data-variant="clip"]').click();
	await expect(area).toBeHidden();
	await page.locator('[data-variant="zoom"]').click();
	await expect.poll(async () => (await area.boundingBox())!.width).toBeCloseTo(after.width, 0);
	await page.screenshot({ path: "test-results/resized-zoom.png" });
});

test("missing model can be downloaded, deleted and downloaded again; errors copy natively", async ({ page }) => {
	await installDesktopBridge(page, "filmstrip.mp4");
	await page.addInitScript(() => {
		const modelPath = "C:\\models\\ggml-small.bin";
		let exists = false;
		localStorage.setItem("devcast.editor.preferences", JSON.stringify({ whisperModelPath: modelPath }));
		Object.assign(window.electronAPI, {
			getCurrentVideoPath: async () => ({ success: true, path: "C:\\recordings\\fixture.mp4" }),
			getLocalMediaUrl: async () => ({ success: true, url: `${location.origin}/tests/ui/fixtures/filmstrip.mp4` }),
			getWhisperSmallModelStatus: async () => ({ success: true, exists, path: modelPath }),
			downloadWhisperSmallModel: async () => { exists = true; return { success: true, path: modelPath }; },
			deleteWhisperSmallModel: async () => { exists = false; return { success: true, path: modelPath }; },
			generateAutoCaptions: async () => ({ success: false, error: "Whisper model not found. Download the model again in Caption settings or select an existing model." }),
			copyTextToClipboard: async (text: string) => { document.documentElement.dataset.copiedError = text; return { success: true }; },
		});
	});
	await page.goto("/?windowType=editor");
	await page.getByRole("radio", { name: "Captions", exact: true }).click();
	const download = page.getByRole("button", { name: "Download Model", exact: true });
	await expect(download).toBeVisible();
	await download.click();
	await page.getByRole("button", { name: "Delete Model", exact: true }).click();
	await expect(download).toBeVisible();
	await download.click();
	await page.getByRole("button", { name: "Generate Captions", exact: true }).first().click();
	await page.getByRole("button", { name: "Copy", exact: true }).click();
	await expect(page.locator("html")).toHaveAttribute("data-copied-error", /Whisper model not found/);
	await expect(download).toBeVisible();
});
