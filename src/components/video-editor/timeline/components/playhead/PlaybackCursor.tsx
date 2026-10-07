import { useTimelineContext } from "dnd-timeline";
import { useEffect, useRef, useState, type RefObject } from "react";
import { cn } from "@/lib/utils";
import { formatPlayheadTime } from "../../core/time";
import { getTimelineEdgePanDelta } from "../../core/edgePan";

import {
	getPlayheadDisplayTime,
	getTimeAtClipSeam,
	type ClipPresentation,
} from "../../core/clipPresentation";

interface PlaybackCursorProps {
	clips: ClipPresentation[];
	currentTimeMs: number;
	videoDurationMs: number;
	onSeek?: (time: number) => void;
	onPanRange?: (deltaMs: number) => void;
	timelineRef: RefObject<HTMLDivElement | null>;
	keyframes?: { id: string; time: number }[];
	isLoading?: boolean;
}

export default function PlaybackCursor({
	clips,
	currentTimeMs,
	videoDurationMs,
	onSeek,
	onPanRange,
	timelineRef,
	keyframes = [],
	isLoading = false,
}: PlaybackCursorProps) {
	const { sidebarWidth, direction, range, valueToPixels, pixelsToValue } = useTimelineContext();
	const sideProperty = direction === "rtl" ? "right" : "left";
	const [isDragging, setIsDragging] = useState(false);
	const [isHovered, setIsHovered] = useState(false);
	const dragClientXRef = useRef<number | null>(null);
	const dragRafRef = useRef<number | null>(null);

	useEffect(() => {
		if (!isDragging) return;

		const updateDrag = () => {
			dragRafRef.current = null;
			if (!timelineRef.current || !onSeek || dragClientXRef.current === null) return;
			const rect = timelineRef.current.getBoundingClientRect();
			const clientX = dragClientXRef.current;
			const clickX =
				direction === "rtl"
					? rect.right - sidebarWidth - clientX
					: clientX - rect.left - sidebarWidth;
			const relativeMs = pixelsToValue(clickX);
			const requestedEdgeDelta =
				onPanRange && range.end - range.start < videoDurationMs
					? getTimelineEdgePanDelta(
							clientX,
							rect,
							sidebarWidth,
							range.end - range.start,
							direction,
						)
					: 0;
			const edgeDelta = Math.max(
				-range.start,
				Math.min(videoDurationMs - range.end, requestedEdgeDelta),
			);
			if (edgeDelta) onPanRange?.(edgeDelta);
			let absoluteMs = getTimeAtClipSeam(
				Math.max(0, Math.min(range.start + relativeMs + edgeDelta, videoDurationMs)),
				clips,
			);

			const snapThresholdMs = 150;
			const nearbyKeyframe = keyframes.find(
				(kf) =>
					Math.abs(kf.time - absoluteMs) <= snapThresholdMs &&
					kf.time >= range.start &&
					kf.time <= range.end,
			);
			if (nearbyKeyframe) absoluteMs = nearbyKeyframe.time;

			onSeek(absoluteMs / 1000);
			if (edgeDelta) dragRafRef.current = requestAnimationFrame(updateDrag);
		};

		const handleMouseMove = (e: MouseEvent) => {
			dragClientXRef.current = e.clientX;
			if (dragRafRef.current === null) dragRafRef.current = requestAnimationFrame(updateDrag);
		};

		const handleMouseUp = () => {
			dragClientXRef.current = null;
			setIsDragging(false);
			document.body.style.cursor = "";
		};

		window.addEventListener("mousemove", handleMouseMove);
		window.addEventListener("mouseup", handleMouseUp);
		if (dragClientXRef.current !== null) {
			dragRafRef.current = requestAnimationFrame(updateDrag);
		}
		document.body.style.cursor = "ew-resize";

		return () => {
			window.removeEventListener("mousemove", handleMouseMove);
			window.removeEventListener("mouseup", handleMouseUp);
			if (dragRafRef.current !== null) cancelAnimationFrame(dragRafRef.current);
			dragRafRef.current = null;
			document.body.style.cursor = "";
		};
	}, [
		isDragging,
		clips,
		onSeek,
		onPanRange,
		timelineRef,
		sidebarWidth,
		range.start,
		range.end,
		videoDurationMs,
		pixelsToValue,
		keyframes,
		direction,
	]);

	if (videoDurationMs <= 0 || currentTimeMs < 0) return null;
	const clampedTime = Math.min(currentTimeMs, videoDurationMs);
	// Keep a playhead visible at the nearest edge even when playback is outside
	// the zoomed timeline range.
	const visibleTime = Math.max(range.start, Math.min(clampedTime, range.end));
	const offset = valueToPixels(getPlayheadDisplayTime(visibleTime, clips) - range.start);

	const expanded = isHovered || isDragging || isLoading;
	// Keep the timestamp inside the viewport even at either end of the timeline.
	const width = valueToPixels(range.end - range.start);
	const capShift = Math.max(0, 34 - offset) - Math.max(0, 34 - (width - offset));
	return (
		<div data-testid="timeline-playhead" className="absolute inset-0 z-50 pointer-events-none">
			<div
				className="absolute top-0 bottom-0 w-px bg-red-500"
				style={{ [sideProperty]: sidebarWidth + offset }}
			>
				<button
					type="button"
					aria-label={`Playhead ${formatPlayheadTime(clampedTime)}`}
					data-testid="playhead-cap"
					onMouseEnter={() => setIsHovered(true)}
					onMouseLeave={() => setIsHovered(false)}
					onFocus={() => setIsHovered(true)}
					onBlur={() => setIsHovered(false)}
					onClick={(event) => event.stopPropagation()}
					onMouseDown={(event) => {
						if (event.button !== 0) return;
						event.preventDefault();
						event.stopPropagation();
						setIsDragging(true);
						dragClientXRef.current = event.clientX;
					}}
					className="absolute top-0 h-4 rounded-full bg-red-500 text-white cursor-ew-resize pointer-events-auto overflow-hidden transition-[width,transform] duration-150"
					style={{
						width: expanded ? 68 : 16,
						left: "50%",
						transform: `translateX(calc(-50% + ${expanded ? (direction === "rtl" ? -capShift : capShift) : 0}px))`,
					}}
				>
					<span
						className={cn(
							"block whitespace-nowrap text-[10px] font-medium tabular-nums transition-opacity",
							expanded ? "opacity-100" : "opacity-0",
						)}
					>
						{formatPlayheadTime(clampedTime)}
					</span>
				</button>
			</div>
		</div>
	);
}
