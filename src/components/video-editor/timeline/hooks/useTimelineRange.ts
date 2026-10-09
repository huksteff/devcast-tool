import type { Range } from "dnd-timeline";
import {
	type RefObject,
	useCallback,
	useEffect,
	useMemo,
	useRef,
	useState,
	type WheelEvent,
} from "react";
import { createInitialRange, normalizeWheelDeltaToPixels } from "../core/time";

interface UseTimelineRangeParams {
	totalMs: number;
	timelineContainerRef: RefObject<HTMLDivElement | null>;
	/** Changes only when a different source is loaded. Item edits must not reset zoom. */
	resetKey?: string | null;
}

export function preserveTimelineRange(range: Range, totalMs: number): Range {
	if (totalMs <= 0) return createInitialRange(totalMs);
	if (!Number.isFinite(range.start) || !Number.isFinite(range.end) || range.end <= range.start) {
		return createInitialRange(totalMs);
	}
	const visibleSpan = Math.max(1, Math.min(range.end - range.start, totalMs));
	const maxStart = Math.max(0, totalMs - visibleSpan);
	const start = Math.max(0, Math.min(range.start, maxStart));
	return { start, end: start + visibleSpan };
}

export interface TimelineWheelPanDeltaInput {
	deltaX: number;
	deltaY: number;
	deltaMode: number;
	shiftKey?: boolean;
	ctrlKey?: boolean;
	metaKey?: boolean;
	canScrollVertically?: boolean;
}

export function resolveTimelineWheelPanDeltaPx({
	deltaX,
	deltaY,
	deltaMode,
	shiftKey = false,
	ctrlKey = false,
	metaKey = false,
	canScrollVertically = true,
}: TimelineWheelPanDeltaInput) {
	if ((ctrlKey || metaKey) && !shiftKey) {
		return 0;
	}

	if (Math.abs(deltaX) > 0) {
		return normalizeWheelDeltaToPixels(deltaX, deltaMode);
	}

	if ((shiftKey || !canScrollVertically) && Math.abs(deltaY) > 0) {
		return normalizeWheelDeltaToPixels(deltaY, deltaMode);
	}

	return 0;
}

export function useTimelineRange({
	totalMs,
	timelineContainerRef,
	resetKey,
}: UseTimelineRangeParams) {
	const [range, setRange] = useState<Range>(() => createInitialRange(totalMs));
	const previousResetKeyRef = useRef(resetKey);
	const previousTotalMsRef = useRef(totalMs);

	useEffect(() => {
		const sourceChanged = previousResetKeyRef.current !== resetKey;
		const durationBecameAvailable = previousTotalMsRef.current <= 0 && totalMs > 0;
		previousResetKeyRef.current = resetKey;
		previousTotalMsRef.current = totalMs;
		setRange((previous) =>
			sourceChanged || durationBecameAvailable
				? createInitialRange(totalMs)
				: preserveTimelineRange(previous, totalMs),
		);
	}, [resetKey, totalMs]);

	const clampedRange = useMemo<Range>(() => {
		if (totalMs === 0) {
			return range;
		}
		return {
			start: Math.max(0, Math.min(range.start, totalMs)),
			end: Math.min(range.end, totalMs),
		};
	}, [range, totalMs]);

	const panTimelineRange = useCallback(
		(deltaMs: number) => {
			if (!Number.isFinite(deltaMs) || deltaMs === 0 || totalMs <= 0) {
				return;
			}

			setRange((previous) => {
				const visibleSpan = Math.max(1, previous.end - previous.start);
				const maxStart = Math.max(0, totalMs - visibleSpan);
				const nextStart = Math.max(0, Math.min(previous.start + deltaMs, maxStart));
				if (nextStart === previous.start) return previous;
				return { start: nextStart, end: nextStart + visibleSpan };
			});
		},
		[totalMs],
	);

	const handleTimelineWheel = useCallback(
		(event: WheelEvent<HTMLDivElement>) => {
			if (((event.ctrlKey || event.metaKey) && !event.shiftKey) || totalMs <= 0) {
				return;
			}

			const container = timelineContainerRef.current;
			const horizontalDeltaPx = resolveTimelineWheelPanDeltaPx({
				deltaX: event.deltaX,
				deltaY: event.deltaY,
				deltaMode: event.deltaMode,
				shiftKey: event.shiftKey,
				ctrlKey: event.ctrlKey,
				metaKey: event.metaKey,
				canScrollVertically: container
					? container.scrollHeight > container.clientHeight + 1
					: true,
			});

			if (horizontalDeltaPx === 0) {
				return;
			}

			const containerWidth = container?.clientWidth ?? 0;
			const visibleRangeMs = clampedRange.end - clampedRange.start;
			if (containerWidth <= 0 || visibleRangeMs <= 0) {
				return;
			}

			event.preventDefault();
			const deltaMs = (horizontalDeltaPx / containerWidth) * visibleRangeMs;
			panTimelineRange(deltaMs);
		},
		[clampedRange.end, clampedRange.start, panTimelineRange, timelineContainerRef, totalMs],
	);

	return {
		range,
		setRange,
		clampedRange,
		panTimelineRange,
		handleTimelineWheel,
	};
}
