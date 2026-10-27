import { describe, expect, it } from "vitest";
import type { AnnotationRegion } from "./types";
import { describeAnnotationKeyframeChanges } from "./keyframeChanges";

describe("keyframe hover summary", () => {
	it("describes property changes against the preceding keyframe", () => {
		const annotation = {
			position: { x: 0, y: 0 }, size: { width: 20, height: 10 },
			style: { color: "#ffffff", fontSize: 14 },
			positionKeyframes: [
				{ id: "first", timeMs: 100, position: { x: 10, y: 0 }, size: { width: 20, height: 10 }, properties: { style: { color: "#ffffff", fontSize: 14 } } },
				{ id: "second", timeMs: 200, position: { x: 10, y: 0 }, size: { width: 30, height: 10 }, properties: { style: { color: "#ff0000", fontSize: 14 } } },
			],
		} as unknown as AnnotationRegion;
		const summary = describeAnnotationKeyframeChanges(annotation);
		expect(summary.get("first")).toContain("Position X: 0 → 10");
		expect(summary.get("second")).toContain("Width: 20 → 30");
		expect(summary.get("second")).toContain("Text color: #ffffff → #ff0000");
	});
});
