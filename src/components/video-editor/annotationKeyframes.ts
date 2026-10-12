import type {
	AnnotationPosition,
	AnnotationPositionKeyframe,
	AnnotationKeyframeProperties,
	AnnotationRegion,
	AnnotationSize,
} from "./types";

export interface AnnotationTransform {
	position: AnnotationPosition;
	size: AnnotationSize;
}

export function resolveAnnotationTransformAtTime(
	annotation: Pick<AnnotationRegion, "position" | "size" | "positionKeyframes">,
	timeMs: number,
): AnnotationTransform {
	const fallback = { position: annotation.position, size: annotation.size ?? { width: 30, height: 20 } };
	const frames = [...(annotation.positionKeyframes ?? [])]
		.filter((frame) => Number.isFinite(frame.timeMs))
		.sort((left, right) => left.timeMs - right.timeMs);
	if (frames.length === 0) return fallback;
	const transformFor = (frame: AnnotationPositionKeyframe): AnnotationTransform => ({
		position: frame.position,
		size: frame.size ?? fallback.size,
	});
	if (timeMs <= frames[0].timeMs) return transformFor(frames[0]);
	const last = frames[frames.length - 1];
	if (timeMs >= last.timeMs) return transformFor(last);
	const rightIndex = frames.findIndex((frame) => frame.timeMs >= timeMs);
	const left = frames[rightIndex - 1];
	const right = frames[rightIndex];
	const progress = (timeMs - left.timeMs) / Math.max(1, right.timeMs - left.timeMs);
	const easedProgress = progress * progress * (3 - 2 * progress);
	const leftTransform = transformFor(left);
	const rightTransform = transformFor(right);
	return {
		position: {
			x: leftTransform.position.x + (rightTransform.position.x - leftTransform.position.x) * easedProgress,
			y: leftTransform.position.y + (rightTransform.position.y - leftTransform.position.y) * easedProgress,
		},
		size: {
			width: leftTransform.size.width + (rightTransform.size.width - leftTransform.size.width) * easedProgress,
			height: leftTransform.size.height + (rightTransform.size.height - leftTransform.size.height) * easedProgress,
		},
	};
}

export function resolveAnnotationPositionAtTime(
	annotation: Pick<AnnotationRegion, "position" | "size" | "positionKeyframes">,
	timeMs: number,
): AnnotationPosition {
	return resolveAnnotationTransformAtTime(annotation, timeMs).position;
}

export function upsertAnnotationPositionKeyframe(
	frames: AnnotationPositionKeyframe[] | undefined,
	timeMs: number,
	position: AnnotationPosition,
	size?: AnnotationSize,
): AnnotationPositionKeyframe[] {
	const roundedTime = Math.round(timeMs);
	const existing = (frames ?? []).find((frame) => Math.abs(frame.timeMs - roundedTime) < 1);
	const next = existing
		? (frames ?? []).map((frame) =>
				frame.id === existing.id
					? { ...frame, timeMs: roundedTime, position, ...(size ? { size } : {}) }
					: frame,
			)
		: [
				...(frames ?? []),
				{ id: globalThis.crypto.randomUUID(), timeMs: roundedTime, position, ...(size ? { size } : {}) },
			];
	return [...next].sort((left, right) => left.timeMs - right.timeMs);
}

export function getAnnotationKeyframeProperties(annotation: AnnotationRegion): AnnotationKeyframeProperties {
	return {
		type: annotation.type,
		content: annotation.content,
		textContent: annotation.textContent,
		imageContent: annotation.imageContent,
		style: annotation.style,
		figureData: annotation.figureData,
		drawingData: annotation.drawingData,
		clickData: annotation.clickData,
		calloutEvent: annotation.calloutEvent,
		stages: annotation.stages,
		blurIntensity: annotation.blurIntensity,
		blurColor: annotation.blurColor,
	};
}

function interpolateColor(left: string, right: string, progress: number): string | null {
	const hex = /^#([\da-f]{6})$/i;
	const leftMatch = left.match(hex);
	const rightMatch = right.match(hex);
	if (!leftMatch || !rightMatch) return null;
	const leftValue = Number.parseInt(leftMatch[1], 16);
	const rightValue = Number.parseInt(rightMatch[1], 16);
	const channels = [16, 8, 0].map((shift) =>
		Math.round(
			((leftValue >> shift) & 255) * (1 - progress) +
			((rightValue >> shift) & 255) * progress,
		),
	);
	return `#${channels.map((channel) => channel.toString(16).padStart(2, "0")).join("")}`;
}

function interpolateSetting(left: unknown, right: unknown, progress: number): unknown {
	if (progress >= 1) return right;
	if (typeof left === "number" && typeof right === "number") {
		return left + (right - left) * progress;
	}
	if (typeof left === "string" && typeof right === "string") {
		return interpolateColor(left, right, progress) ?? left;
	}
	if (Array.isArray(left) && Array.isArray(right) && left.length === right.length) {
		return left.map((value, index) => interpolateSetting(value, right[index], progress));
	}
	if (left && right && typeof left === "object" && typeof right === "object" && !Array.isArray(left) && !Array.isArray(right)) {
		const leftRecord = left as Record<string, unknown>;
		const rightRecord = right as Record<string, unknown>;
		return Object.fromEntries(
			Object.keys(leftRecord).map((key) => [
				key,
				key in rightRecord
					? interpolateSetting(leftRecord[key], rightRecord[key], progress)
					: leftRecord[key],
			]),
		);
	}
	return left;
}

export function resolveAnnotationPropertiesAtTime(
	annotation: AnnotationRegion,
	timeMs: number,
): AnnotationKeyframeProperties {
	const fallback = getAnnotationKeyframeProperties(annotation);
	const expand = (properties: AnnotationKeyframeProperties | undefined): AnnotationKeyframeProperties => ({
		...fallback,
		...properties,
		style: { ...fallback.style, ...properties?.style },
	});
	const frames = (annotation.positionKeyframes ?? [])
		.filter((frame) => frame.properties && Number.isFinite(frame.timeMs))
		.sort((left, right) => left.timeMs - right.timeMs);
	if (frames.length === 0) return fallback;
	if (timeMs <= frames[0].timeMs) {
		if (frames[0].timeMs <= annotation.startMs) return expand(frames[0].properties);
		if (timeMs === frames[0].timeMs) return expand(frames[0].properties);
		const progress = Math.max(0, Math.min(1, (timeMs - annotation.startMs) / (frames[0].timeMs - annotation.startMs)));
		const easedProgress = progress * progress * (3 - 2 * progress);
		return interpolateSetting(fallback, expand(frames[0].properties), easedProgress) as AnnotationKeyframeProperties;
	}
	const last = frames[frames.length - 1];
	if (timeMs >= last.timeMs) return expand(last.properties);
	const rightIndex = frames.findIndex((frame) => frame.timeMs >= timeMs);
	const left = frames[rightIndex - 1];
	const right = frames[rightIndex];
	const progress = (timeMs - left.timeMs) / Math.max(1, right.timeMs - left.timeMs);
	const easedProgress = progress * progress * (3 - 2 * progress);
	return interpolateSetting(expand(left.properties), expand(right.properties), easedProgress) as AnnotationKeyframeProperties;
}

export function resolveAnnotationAtTime(annotation: AnnotationRegion, timeMs: number): AnnotationRegion {
	if (!(annotation.positionKeyframes ?? []).some((frame) => frame.properties)) return annotation;
	return { ...annotation, ...resolveAnnotationPropertiesAtTime(annotation, timeMs) };
}

export function applyManualAnnotationTransform(
	annotation: AnnotationRegion,
	timeMs: number,
	change: Partial<AnnotationTransform>,
): AnnotationRegion {
	if (!Number.isFinite(timeMs)) return annotation;
	const time = Math.max(annotation.startMs, Math.min(annotation.endMs, Math.round(timeMs)));
	const resolved = resolveAnnotationTransformAtTime(annotation, time);
	const position = change.position ?? resolved.position;
	const size = change.size ?? resolved.size;
	if (
		position.x === resolved.position.x &&
		position.y === resolved.position.y &&
		size.width === resolved.size.width &&
		size.height === resolved.size.height
	) return annotation;
	let frames = annotation.positionKeyframes ?? [];
	if (frames.length === 0 && time > annotation.startMs) {
		frames = upsertAnnotationPositionKeyframe(
			frames,
			annotation.startMs,
			annotation.position,
			annotation.size,
		);
	}
	return {
		...annotation,
		positionKeyframes: upsertAnnotationPositionKeyframe(frames, time, position, size),
	};
}

export function applyManualAnnotationProperties(
	annotation: AnnotationRegion,
	timeMs: number,
	change: Partial<AnnotationKeyframeProperties>,
): AnnotationRegion {
	if (!Number.isFinite(timeMs)) return annotation;
	const time = Math.max(annotation.startMs, Math.min(annotation.endMs, Math.round(timeMs)));
	const current = resolveAnnotationPropertiesAtTime(annotation, time);
	const changed = Object.entries(change).some(
		([key, value]) =>
			JSON.stringify(current[key as keyof AnnotationKeyframeProperties]) !== JSON.stringify(value),
	);
	if (!changed) return annotation;
	const properties = structuredClone({ ...current, ...change });
	const transform = resolveAnnotationTransformAtTime(annotation, time);
	const frames = upsertAnnotationPositionKeyframe(annotation.positionKeyframes, time, transform.position, transform.size)
		.map((frame) => frame.timeMs === time ? { ...frame, properties } : frame);
	return { ...annotation, positionKeyframes: frames };
}
