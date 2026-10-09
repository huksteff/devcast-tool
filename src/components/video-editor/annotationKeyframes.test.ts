import { describe, expect, it, vi } from "vitest";
import { resolveAnnotationPositionAtTime, resolveAnnotationTransformAtTime, upsertAnnotationPositionKeyframe } from "./annotationKeyframes";

describe("annotation position keyframes", () => {
	it("interpolates position between frames", () => {
		expect(
			resolveAnnotationPositionAtTime(
				{
					position: { x: 0, y: 0 },
					positionKeyframes: [
						{ id: "a", timeMs: 1000, position: { x: 10, y: 20 } },
						{ id: "b", timeMs: 3000, position: { x: 50, y: 60 } },
					],
				},
				2000,
			),
		).toEqual({ x: 30, y: 40 });
	});

	it("uses eased interpolation to avoid a velocity jump at slow keyframes", () => {
		const annotation = {
			position: { x: 0, y: 0 },
			positionKeyframes: [
				{ id: "a", timeMs: 1000, position: { x: 0, y: 0 } },
				{ id: "b", timeMs: 3000, position: { x: 100, y: 100 } },
			],
		};
		expect(resolveAnnotationPositionAtTime(annotation, 1500)).toEqual({ x: 15.625, y: 15.625 });
		expect(resolveAnnotationPositionAtTime(annotation, 2000)).toEqual({ x: 50, y: 50 });
	});

	it("holds the first and last values outside the keyframe interval", () => {
		const annotation = {
			position: { x: 0, y: 0 },
			positionKeyframes: [
				{ id: "a", timeMs: 1000, position: { x: 10, y: 20 } },
				{ id: "b", timeMs: 3000, position: { x: 50, y: 60 } },
			],
		};
		expect(resolveAnnotationPositionAtTime(annotation, 0)).toEqual({ x: 10, y: 20 });
		expect(resolveAnnotationPositionAtTime(annotation, 4000)).toEqual({ x: 50, y: 60 });
	});

	it("interpolates size with position for transform keyframes", () => {
		const annotation = {
			position: { x: 0, y: 0 },
			size: { width: 10, height: 20 },
			positionKeyframes: [
				{ id: "a", timeMs: 0, position: { x: 0, y: 0 }, size: { width: 10, height: 20 } },
				{ id: "b", timeMs: 1000, position: { x: 100, y: 100 }, size: { width: 50, height: 60 } },
			],
		};
		expect(resolveAnnotationTransformAtTime(annotation, 500)).toEqual({
			position: { x: 50, y: 50 },
			size: { width: 30, height: 40 },
		});
	});

	it("updates an existing frame at the same time", () => {
		vi.stubGlobal("crypto", { randomUUID: () => "new" });
		expect(
			upsertAnnotationPositionKeyframe(
				[{ id: "a", timeMs: 1000, position: { x: 10, y: 20 } }],
				1000,
				{ x: 30, y: 40 },
			),
		).toEqual([{ id: "a", timeMs: 1000, position: { x: 30, y: 40 } }]);
		vi.unstubAllGlobals();
	});
});
