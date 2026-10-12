import { describe, expect, it } from "vitest";
import {
	eraseDrawingAtPoint,
	getAnimatedGradientColors,
	getAnnotationStageState,
	resolveCursorCalloutState,
	type AnnotationRegion,
} from "./types";

const annotation = (transition: "slide-left" | "slide-right"): AnnotationRegion => ({
	id: "annotation",
	startMs: 0,
	endMs: 1000,
	type: "text",
	content: "text",
	position: { x: 0, y: 0 },
	size: { width: 20, height: 10 },
	style: {
		color: "#fff",
		backgroundColor: "transparent",
		fontSize: 32,
		fontFamily: "sans-serif",
		fontWeight: "normal",
		fontStyle: "normal",
		textDecoration: "none",
		textAlign: "left",
		borderRadius: 0,
	},
	zIndex: 0,
	stages: {
		inEnabled: true,
		inDurationMs: 500,
		inTransition: transition,
		outEnabled: false,
		outDurationMs: 0,
		outTransition: "none",
	},
});

describe("annotation model", () => {
	it("slides horizontally from the selected side", () => {
		expect(getAnnotationStageState(annotation("slide-left"), 0).translateX).toBeLessThan(0);
		expect(getAnnotationStageState(annotation("slide-right"), 0).translateX).toBeGreaterThan(0);
		expect(getAnnotationStageState(annotation("slide-left"), 500).translateX).toBe(0);
	});

	it("gives transform transitions a distinct visual state instead of a fade", () => {
		const base = annotation("slide-left");
		const pop = getAnnotationStageState(
			{ ...base, stages: { ...base.stages!, inTransition: "pop" } },
			200,
		);
		const zoom = getAnnotationStageState(
			{ ...base, stages: { ...base.stages!, inTransition: "zoom" } },
			200,
		);
		const swing = getAnnotationStageState(
			{ ...base, stages: { ...base.stages!, inTransition: "swing" } },
			200,
		);
		expect(pop.scale).not.toBe(1);
		expect(zoom.scale).not.toBe(pop.scale);
		expect(swing.rotate).not.toBe(0);
	});

	it("erases only the touched area and keeps remaining stroke fragments", () => {
		const result = eraseDrawingAtPoint(
			{
				points: [],
				strokes: [
					[
						{ x: 0, y: 0 },
						{ x: 20, y: 0 },
						{ x: 40, y: 0 },
					],
				],
				color: "#fff",
				strokeWidth: 4,
			},
			{ x: 20, y: 0 },
			5,
		);
		expect(result.strokes).toEqual([[{ x: 0, y: 0 }], [{ x: 40, y: 0 }]]);
	});

	it("plays callout enable, text and erase events from timeline time", () => {
		const event = (id: string, startMs: number, endMs: number, action: "enable" | "text" | "disable", content = ""): AnnotationRegion => ({
			...annotation("slide-left"), id, startMs, endMs, type: "cursor-callout", content, calloutEvent: { action },
		});
		const events = [event("on", 0, 100, "enable"), event("one", 100, 300, "text", "Hello"), event("off", 300, 500, "disable")];
		expect(resolveCursorCalloutState(events, 50, { enabled: false, text: "fallback" })).toEqual({ enabled: true, text: "fallback" });
		expect(resolveCursorCalloutState(events, 200, { enabled: false, text: "fallback" })).toEqual({ enabled: true, text: "Hel" });
		expect(resolveCursorCalloutState(events, 400, { enabled: false, text: "fallback" })).toEqual({ enabled: true, text: "Hel" });
		expect(resolveCursorCalloutState(events, 500, { enabled: false, text: "fallback" })).toEqual({ enabled: false, text: "" });
		expect(resolveCursorCalloutState(events, 50, { enabled: false, text: "Base" })).toEqual({ enabled: true, text: "Base" });
		expect(resolveCursorCalloutState([
			event("on", 0, 100, "enable"), event("empty", 100, 300, "text"),
		], 200, { enabled: false, text: "" })).toEqual({ enabled: true, text: "Te" });
	});

	it("keeps the previous label visible at a text change and carries event style forward", () => {
		const base = annotation("slide-left");
		const events: AnnotationRegion[] = [
			{ ...base, id: "on", type: "cursor-callout", startMs: 0, endMs: 100, calloutEvent: { action: "enable", style: { background: "#FF0000", gradientEnabled: true, gradientColors: ["#FF0000", "#0000FF"] } } },
			{ ...base, id: "first", type: "cursor-callout", startMs: 100, endMs: 300, content: "Hello", calloutEvent: { action: "text" } },
			{ ...base, id: "second", type: "cursor-callout", startMs: 300, endMs: 500, content: "World", calloutEvent: { action: "text", style: { textColor: "#00FF00" } } },
		];
		expect(resolveCursorCalloutState(events, 300, { enabled: false, text: "" })).toEqual({
			enabled: true, text: "Hello",
			style: { background: "#FF0000", gradientEnabled: true, gradientColors: ["#FF0000", "#0000FF"], textColor: "#00FF00" },
		});
		expect(resolveCursorCalloutState(events, 301, { enabled: false, text: "" }).text).toBe("W");
	});

	it("interpolates gradient colors continuously and closes the cycle without a jump", () => {
		const gradient = {
			enabled: true,
			colors: ["#FF0000", "#00FF00", "#0000FF"],
			cycleDurationMs: 300,
			glow: 12,
		};
		const first = getAnimatedGradientColors(gradient, 0, "#FFFFFF");
		expect(first).toHaveLength(9);
		expect(first[0]).toBe("#ff0000");
		expect(first[8]).toBe(first[0]);
		expect(getAnimatedGradientColors(gradient, 300, "#FFFFFF")).toEqual(first);
		expect(getAnimatedGradientColors(gradient, 100, "#FFFFFF")[0]).toBe("#00ff00");
		expect(getAnimatedGradientColors(gradient, 101, "#FFFFFF")[0]).not.toBe("#0000ff");
		const twoColors = { ...gradient, colors: ["#FF0000", "#0000FF"] };
		expect(getAnimatedGradientColors(twoColors, 75, "#FFFFFF")[0]).toBe("#800080");
		expect(getAnimatedGradientColors(twoColors, 299, "#FFFFFF")[0]).not.toBe("#0000ff");
	});
});
