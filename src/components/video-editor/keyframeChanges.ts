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

function collectChanges(before: unknown, after: unknown, path: string, output: string[]): void {
	if (Object.is(before, after)) return;
	if (before && after && typeof before === "object" && typeof after === "object" && !Array.isArray(before) && !Array.isArray(after)) {
		for (const key of new Set([...Object.keys(before), ...Object.keys(after)])) {
			collectChanges((before as Record<string, unknown>)[key], (after as Record<string, unknown>)[key], path ? `${path}.${key}` : key, output);
		}
		return;
	}
	if (JSON.stringify(before) !== JSON.stringify(after)) output.push(`${path}: ${readable(before)} → ${readable(after)}`);
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
