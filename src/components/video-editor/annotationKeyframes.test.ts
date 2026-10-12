import { describe, expect, it, vi } from "vitest";
import { applyManualAnnotationProperties, applyManualAnnotationTransform, resolveAnnotationAtTime, resolveAnnotationPositionAtTime, resolveAnnotationTransformAtTime, upsertAnnotationPositionKeyframe } from "./annotationKeyframes";
import type { AnnotationRegion } from "./types";

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

	it("creates a keyframe on the first small manual change and preserves the starting transform", () => {
		const annotation = {
			id: "highlight",
			startMs: 7000,
			endMs: 14000,
			position: { x: 50, y: 50 },
			size: { width: 30, height: 20 },
		} as AnnotationRegion;
		vi.stubGlobal("crypto", { randomUUID: vi.fn().mockReturnValueOnce("anchor").mockReturnValueOnce("edited") });
		const changed = applyManualAnnotationTransform(annotation, 8800, { position: { x: 50.001, y: 50 } });
		expect(changed.positionKeyframes).toEqual([
			{ id: "anchor", timeMs: 7000, position: annotation.position, size: annotation.size },
			{ id: "edited", timeMs: 8800, position: { x: 50.001, y: 50 }, size: annotation.size },
		]);
		expect(resolveAnnotationTransformAtTime(changed, 7000).position).toEqual(annotation.position);
		vi.unstubAllGlobals();
	});

	it("updates the current keyframe during resizing without replacing its position or id", () => {
		const annotation = {
			id: "highlight",
			startMs: 7000,
			endMs: 14000,
			position: { x: 50, y: 50 },
			size: { width: 30, height: 20 },
			positionKeyframes: [
				{ id: "existing", timeMs: 8800, position: { x: 60, y: 70 }, size: { width: 30, height: 20 } },
			],
		} as AnnotationRegion;
		const changed = applyManualAnnotationTransform(annotation, 8800, { size: { width: 30.001, height: 20 } });
		expect(changed.positionKeyframes).toEqual([
			{ id: "existing", timeMs: 8800, position: { x: 60, y: 70 }, size: { width: 30.001, height: 20 } },
		]);
	});

	it("records a style change at the playhead and interpolates color and size while text stays discrete", () => {
		const annotation = {
			id: "text",
			startMs: 1000,
			endMs: 5000,
			type: "text",
			content: "Before",
			position: { x: 50, y: 50 },
			size: { width: 30, height: 20 },
			style: { color: "#000000", fontSize: 20 },
		} as AnnotationRegion;
		vi.stubGlobal("crypto", { randomUUID: () => "edited" });
		const changed = applyManualAnnotationProperties(annotation, 3000, {
			content: "After",
			style: { ...annotation.style, color: "#ffffff", fontSize: 40 },
		});
		expect(changed.positionKeyframes?.map((frame) => [frame.id, frame.timeMs])).toEqual([["edited", 3000]]);
		expect(resolveAnnotationAtTime(changed, 2000)).toMatchObject({
			content: "Before",
			style: { color: "#808080", fontSize: 30 },
		});
		expect(resolveAnnotationAtTime(changed, 3000)).toMatchObject({
			content: "After",
			style: { color: "#ffffff", fontSize: 40 },
		});
		vi.unstubAllGlobals();
	});

	it("resolves the first of multiple property keyframes without accessing a missing previous frame", () => {
		const annotation = {
			id: "text",
			startMs: 0,
			endMs: 2000,
			type: "text",
			content: "Base",
			position: { x: 50, y: 50 },
			size: { width: 30, height: 20 },
			style: { color: "#000000", fontSize: 20 },
			positionKeyframes: [
				{ id: "first", timeMs: 0, position: { x: 50, y: 50 }, size: { width: 30, height: 20 }, properties: { content: "First", style: { color: "#ff0000", fontSize: 24 } } },
				{ id: "second", timeMs: 1000, position: { x: 50, y: 50 }, size: { width: 40, height: 25 }, properties: { content: "Second", style: { color: "#0000ff", fontSize: 30 } } },
			],
		} as AnnotationRegion;
		expect(resolveAnnotationAtTime(annotation, 0)).toMatchObject({ content: "First", style: { color: "#ff0000" } });
		expect(resolveAnnotationAtTime(annotation, 500)).toMatchObject({ content: "First", style: { color: "#800080" } });
		expect(resolveAnnotationAtTime(annotation, 1000)).toMatchObject({ content: "Second", style: { color: "#0000ff" } });
	});

	it("edits an existing manual keyframe without creating another marker", () => {
		const annotation = {
			id: "text",
			startMs: 0,
			endMs: 2000,
			type: "text",
			content: "Before",
			position: { x: 50, y: 50 },
			size: { width: 30, height: 20 },
			style: { color: "#000000", fontSize: 20 },
			positionKeyframes: [{
				id: "manual",
				timeMs: 1000,
				position: { x: 50, y: 50 },
				size: { width: 30, height: 20 },
			}],
		} as AnnotationRegion;
		const changed = applyManualAnnotationProperties(annotation, 1000, { content: "After" });
		expect(changed.positionKeyframes).toHaveLength(1);
		expect(changed.positionKeyframes?.[0]).toMatchObject({
			id: "manual",
			timeMs: 1000,
			properties: { content: "After" },
		});
	});
});
