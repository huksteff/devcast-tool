import { describe, expect, it } from "vitest";
import { compareAnnotationLayerOrder } from "./annotationLayerOrder";

describe("annotation layer order", () => {
	it("paints lower timeline lanes above upper lanes even when their legacy z-index is lower", () => {
		const layers = [
			{ id: "lower", trackIndex: 2, zIndex: 0 },
			{ id: "upper", trackIndex: 0, zIndex: 100 },
			{ id: "middle", trackIndex: 1, zIndex: 5 },
		].sort(compareAnnotationLayerOrder);
		expect(layers.map((layer) => layer.id)).toEqual(["upper", "middle", "lower"]);
	});
});
