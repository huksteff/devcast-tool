import { expect, test } from "@playwright/test";
import { installDesktopBridge } from "./bridge";

test.use({ channel: process.env.DEVCAST_UI_BROWSER_CHANNEL === "chrome" ? "chrome" : undefined });

test("manual annotation resize survives keyframe changes and scrubbing", async ({ page }) => {
	test.setTimeout(120000);
	const errors: string[] = [];
	page.on("pageerror", (error) => {
		errors.push(error.message);
		console.error("Editor page error:", error.message);
	});
	await installDesktopBridge(page);
	await page.goto("/?windowType=editor");
	await expect(page.getByRole("navigation", { name: "Editor tools" })).toBeVisible({ timeout: 20000 });
	await expect.poll(() => page.locator("video").evaluateAll((videos) => videos.some((video) => (video as HTMLVideoElement).readyState >= 2))).toBe(true);
	await page.getByRole("button", { name: "Add Layer", exact: true }).click();
	await page.getByRole("menuitem", { name: "Annotation", exact: true }).click();
	const annotation = page.locator("[data-annotation-id]").first();
	await expect(annotation).toBeVisible();
	await annotation.click();
	await page.keyboard.press("i");
	await expect(page.getByTitle(/Keyframe @ .*ms/)).toHaveCount(1);
	const firstWidth = (await annotation.boundingBox())!.width;
	const annotationBox = (await annotation.boundingBox())!;
	const handleX = annotationBox.x + annotationBox.width;
	const handleY = annotationBox.y + annotationBox.height;
	await page.mouse.move(handleX, handleY);
	await page.mouse.down();
	await page.mouse.move(handleX + 40, handleY + 15, { steps: 4 });
	await page.mouse.up();
	await expect.poll(async () => (await annotation.boundingBox())!.width).toBeGreaterThan(firstWidth + 15);
	for (let step = 0; step < 20; step += 1) await page.keyboard.press("ArrowRight");
	await page.keyboard.press("i");
	await expect(page.getByTitle(/Keyframe @ .*ms/)).toHaveCount(2);
	const secondWidth = (await annotation.boundingBox())!.width;
	const secondBox = (await annotation.boundingBox())!;
	await page.mouse.move(secondBox.x + secondBox.width, secondBox.y + secondBox.height);
	await page.mouse.down();
	await page.mouse.move(secondBox.x + secondBox.width + 35, secondBox.y + secondBox.height + 10, { steps: 4 });
	await page.mouse.up();
	await expect.poll(async () => (await annotation.boundingBox())!.width).toBeGreaterThan(secondWidth + 10);
	const markers = page.getByTitle(/Keyframe @ .*ms/);
	await markers.nth(0).click();
	await expect.poll(async () => (await annotation.boundingBox())!.width).toBeLessThan(secondWidth + 10);
	await markers.nth(1).click();
	await expect.poll(async () => (await annotation.boundingBox())!.width).toBeGreaterThan(secondWidth + 10);
	await expect(page.getByRole("navigation", { name: "Editor tools" })).toBeVisible();
	expect(errors).toEqual([]);
});

test("cursor label event exposes its own animated gradient style", async ({ page }) => {
	await installDesktopBridge(page);
	await page.goto("/?windowType=editor");
	await expect(page.getByRole("navigation", { name: "Editor tools" })).toBeVisible({ timeout: 20000 });
	await page.getByRole("button", { name: "Add Layer", exact: true }).click();
	await page.getByRole("menuitem", { name: "Annotation", exact: true }).click();
	await page.getByRole("row", { name: "Cursor label event" }).click();
	await expect(page.getByText("Label style", { exact: true })).toBeVisible();
	await page.getByRole("button", { name: "Animated gradient" }).click();
	await expect(page.getByText("Gradient colors", { exact: true })).toBeVisible();
});
