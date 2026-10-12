import { expect, test } from "@playwright/test";
import { installDesktopBridge } from "./bridge";

test.use({ channel: process.env.DEVCAST_UI_BROWSER_CHANNEL === "chrome" ? "chrome" : undefined });

test("closed figures expose fill gradient, opacity and outline controls", async ({ page }) => {
	const errors: string[] = [];
	page.on("pageerror", (error) => errors.push(error.message));
	await installDesktopBridge(page);
	await page.goto("/?windowType=editor");
	await expect(page.getByRole("navigation", { name: "Editor tools" })).toBeVisible({ timeout: 20000 });
	await page.getByRole("button", { name: "Add Layer", exact: true }).click();
	await page.getByRole("menuitem", { name: "Annotation", exact: true }).click();
	await page.getByRole("row", { name: "Arrow" }).click();
	await page.getByRole("row", { name: "Rectangle" }).click();
	await expect(page.getByRole("slider", { name: "Opacity" })).toBeVisible();
	await page.getByRole("button", { name: "Fill", exact: true }).click();
	await page.getByRole("row", { name: "Animated gradient" }).click();
	await expect(page.getByRole("button", { name: "Fill color" })).toBeVisible();
	await expect(page.getByRole("button", { name: "Outline", exact: true })).toBeVisible();
	await page.screenshot({ path: "test-results/figure-gradient-preview.png", animations: "disabled" });
	const exportedPixels = await page.evaluate(async () => {
		const modulePath = "/src/lib/exporter/annotationRenderer.ts";
		const { renderAnnotationToCanvas } = await import(modulePath);
		const canvas = await renderAnnotationToCanvas({
			type: "figure", startMs: 0,
			figureData: {
				kind: "rectangle", arrowDirection: "right", color: "#000000", strokeWidth: 4,
				opacity: 0.5, fillEnabled: true, outlineEnabled: false,
				fillGradient: { enabled: true, colors: ["#ff0000", "#0000ff"], cycleDurationMs: 2400, glow: 0 },
			},
		} as never, 100, 100);
		const legacyHighlight = await renderAnnotationToCanvas({
			type: "figure", startMs: 0,
			figureData: {
				kind: "highlight", arrowDirection: "right", color: "#000000", strokeWidth: 4,
				gradient: { enabled: true, colors: ["#ff0000", "#0000ff"], cycleDurationMs: 2400, glow: 0 },
			},
		} as never, 100, 100);
		const legacyContext = legacyHighlight!.getContext("2d")!;
		return {
			fill: Array.from(canvas!.getContext("2d")!.getImageData(25, 50, 1, 1).data),
			legacyLeft: Array.from(legacyContext.getImageData(25, 50, 1, 1).data),
			legacyCenter: Array.from(legacyContext.getImageData(50, 50, 1, 1).data),
		};
	});
	expect(exportedPixels.fill[3]).toBeGreaterThan(100);
	expect(exportedPixels.fill[3]).toBeLessThan(155);
	expect(exportedPixels.legacyLeft[3]).toBeGreaterThan(50);
	expect(exportedPixels.legacyLeft.slice(0, 3)).not.toEqual(exportedPixels.legacyCenter.slice(0, 3));
	expect(errors).toEqual([]);
});
