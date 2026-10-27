import { getAnnotationKeyframeProperties } from "./annotationKeyframes";
import type { AnnotationPositionKeyframe, AnnotationRegion } from "./types";

function readable(value: unknown): string {
	if (value === undefined || value === null) return "—";
	if (typeof value === "number") return Number.isInteger(value) ? String(value) : value.toFixed(2);
	if (typeof value === "boolean") return value ? "On" : "Off";
	if (typeof value === "string") return value.length > 32 ? `${value.slice(0, 29)}…` : value;
	if (Array.isArray(value)) return value.map(readable).join(", ");
	return "Changed";
}

function labelForPath(path: string): string {
	const parts = path.split(".").filter((part) => part !== "Settings");
	const key = parts[parts.length - 1] ?? path;
	if (parts[0] === "Position") return key === "x" ? "Position X" : "Position Y";
	if (parts[0] === "Size") return key === "width" ? "Width" : "Height";
	const group = parts[0];
	const parent = parts[parts.length - 2];
	if (group === "style" && key === "color") return "Text color";
	const names: Record<string, string> = {
		type: "Annotation type", content: "Text", textContent: "Text", imageContent: "Image",
		figureData: "Shape", drawingData: "Drawing", clickData: "Click effect", calloutEvent: "Callout", stages: "Animation",
		style: "Text style", blurIntensity: "Blur strength", blurColor: "Blur color",
		kind: "Shape", arrowDirection: "Arrow direction", strokeWidth: "Stroke width", opacity: "Opacity",
		fillEnabled: "Fill", fillColor: "Fill color", outlineEnabled: "Outline", color: "Color",
		fontSize: "Font size", fontFamily: "Font", fontWeight: "Font weight", fontStyle: "Font style",
		textDecoration: "Underline", textAlign: "Text alignment", backgroundColor: "Background color", borderRadius: "Corner radius",
		particleCount: "Particle count", radius: "Radius", durationMs: "Duration", cycleDurationMs: "Cycle duration",
		glow: "Glow", colors: "Colors", enabled: "Enabled", gradientEnabled: "Gradient", gradientColors: "Gradient colors",
		gradientGlow: "Gradient glow", scale: "Scale", background: "Background", textColor: "Text color",
		inEnabled: "Entrance", outEnabled: "Exit", inDurationMs: "Entrance duration", outDurationMs: "Exit duration",
		inTransition: "Entrance effect", outTransition: "Exit effect", action: "Action", tool: "Tool",
		eraserSize: "Eraser size", showEraserInDrawOut: "Show eraser on exit", strokes: "Strokes", points: "Points",
	};
	if (parent === "fillGradient" || parent === "gradient") {
		const prefix = parent === "fillGradient" ? "Fill gradient" : "Outline gradient";
		if (key === "enabled") return prefix;
		return `${prefix} ${names[key]?.toLowerCase() ?? key}`;
	}
	const prefix: Record<string, string> = { figureData: "Shape", drawingData: "Drawing", clickData: "Click effect", calloutEvent: "Callout", stages: "Animation" };
	const suffix = names[key] ?? key.replace(/([a-z])([A-Z])/g, "$1 $2").toLowerCase();
	return group && prefix[group] && !suffix.startsWith(prefix[group]) ? `${prefix[group]} ${suffix.toLowerCase()}` : suffix;
}

function collectChanges(before: unknown, after: unknown, path: string, output: string[]): void {
	if (Object.is(before, after)) return;
	const beforeObject = before && typeof before === "object" && !Array.isArray(before) ? before as Record<string, unknown> : null;
	const afterObject = after && typeof after === "object" && !Array.isArray(after) ? after as Record<string, unknown> : null;
	if ((beforeObject || afterObject) && (beforeObject || before == null) && (afterObject || after == null)) {
		for (const key of new Set([...Object.keys(beforeObject ?? {}), ...Object.keys(afterObject ?? {})])) {
			collectChanges(beforeObject?.[key], afterObject?.[key], path ? `${path}.${key}` : key, output);
		}
		return;
	}
	if (JSON.stringify(before) !== JSON.stringify(after)) output.push(`${labelForPath(path)}: ${readable(before)} → ${readable(after)}`);
}

export function describeAnnotationKeyframeChanges(annotation: AnnotationRegion): Map<string, string[]> {
	const changes = new Map<string, string[]>();
	const frames = [...(annotation.positionKeyframes ?? [])].sort((a, b) => a.timeMs - b.timeMs);
	let previous: Pick<AnnotationPositionKeyframe, "position" | "size" | "properties"> = {
		position: annotation.position,
		size: annotation.size,
		properties: getAnnotationKeyframeProperties(annotation),
	};
	for (const frame of frames) {
		const lines: string[] = [];
		collectChanges(previous.position, frame.position, "Position", lines);
		collectChanges(previous.size, frame.size ?? annotation.size, "Size", lines);
		if (frame.properties) collectChanges(previous.properties, frame.properties, "Settings", lines);
		changes.set(frame.id, lines.length ? lines : ["No parameter changes"]);
		previous = { position: frame.position, size: frame.size ?? annotation.size, properties: frame.properties ?? previous.properties };
	}
	return changes;
}
