import { describe, expect, it } from "vitest";
import { getAnnotationContentScale } from "./annotationRenderer";

describe("getAnnotationContentScale", () => {
	it("scales annotation content from the recording rectangle, not the outer export canvas", () => {
		expect(getAnnotationContentScale(1920)).toBe(1);
		expect(getAnnotationContentScale(1440)).toBe(0.75);
	});
});
