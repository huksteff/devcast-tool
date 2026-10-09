import type {
	AnnotationPosition,
	AnnotationPositionKeyframe,
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
