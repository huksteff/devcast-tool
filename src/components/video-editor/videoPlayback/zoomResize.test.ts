import { expect, it } from "vitest";
import { getZoomScale } from "../types";
import { resizeZoomArea } from "./zoomResize";

it("zooms into the dragged rectangle and keeps the opposite corner fixed", () => {
	const result = resizeZoomArea({ cx: 0.1, cy: 0.2 }, { cx: 0.5, cy: 0.6 }, "se");
	expect(result.scale).toBeCloseTo(2.5);
	expect(result.focus.cx).toBeCloseTo(0.3);
	expect(result.focus.cy).toBeCloseTo(0.4);
	expect(getZoomScale({ depth: 2, scale: result.scale })).toBeCloseTo(2.5);
});
it("clamps all four handles within the video and limits magnification", () => {
	for (const corner of ["nw", "ne", "sw", "se"] as const) {
		const anchor = { cx: corner.endsWith("w") ? 0.9 : 0.1, cy: corner.startsWith("n") ? 0.9 : 0.1 };
		for (const pointer of [{ cx: -5, cy: -5 }, { cx: 5, cy: 5 }, anchor]) {
			const result = resizeZoomArea(anchor, pointer, corner);
			const half = 1 / result.scale / 2;
			expect(result.scale).toBeGreaterThanOrEqual(1);
			expect(result.scale).toBeLessThanOrEqual(5);
			expect(result.focus.cx - half).toBeGreaterThanOrEqual(-1e-10);
			expect(result.focus.cy - half).toBeGreaterThanOrEqual(-1e-10);
			expect(result.focus.cx + half).toBeLessThanOrEqual(1 + 1e-10);
			expect(result.focus.cy + half).toBeLessThanOrEqual(1 + 1e-10);
		}
	}
});
