import { describe, expect, it } from "vitest";
import { getTimelineEdgePanDelta } from "./edgePan";

describe("timeline edge pan", () => {
	const rect = { left: 0, right: 1000 } as DOMRect;
	it("moves continuously toward the held edge and stays still in the center", () => {
		expect(getTimelineEdgePanDelta(110, rect, 100, 5_000, "ltr")).toBeLessThan(0);
		expect(getTimelineEdgePanDelta(995, rect, 100, 5_000, "ltr")).toBeGreaterThan(0);
		expect(getTimelineEdgePanDelta(500, rect, 100, 5_000, "ltr")).toBe(0);
	});
});
