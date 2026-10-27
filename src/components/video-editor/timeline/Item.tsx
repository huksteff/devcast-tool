import { useDndMonitor } from "@dnd-kit/core";
import {
	ArrowRight,
	BoundingBox,
	Gauge,
	ImageSquare,
	ChatCircle as MessageSquare,
	MusicNotes as Music,
	PencilSimple,
	Scissors,
	SpeakerX,
	TextT,
	MagnifyingGlassPlus as ZoomIn,
} from "@phosphor-icons/react";
import type { GetSpanFromDragEvent, GetSpanFromResizeEvent, Span } from "dnd-timeline";
import { useItem, useTimelineContext } from "dnd-timeline";
import { useCallback, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Skeleton } from "@/components/ui/skeleton";
import { cn } from "@/lib/utils";
import { formatClipSpeedLabel } from "../clipSpeedChange";
import type { AnnotationStages, AnnotationType } from "../types";
import { ClipFilmstrip } from "./components/filmstrip/ClipFilmstrip";
import AudioWaveform from "./components/waveform/AudioWaveform";
import {
	type ClipPresentation,
	getRegionDisplaySpan,
	getTimeAtClipSeam,
	snapRegionSpan,
} from "./core/clipPresentation";
import { useTimelinePresentation } from "./core/TimelinePresentation";
import { formatPlayheadTime } from "./core/time";
import type { AudioPeaksData } from "./core/timelineTypes";
import { resolveDragEnd, resolveResizeEnd } from "./dnd/engine";
import glassStyles from "./ItemGlass.module.css";

interface ItemProps {
	clipPresentation?: ClipPresentation[];
	sharedLeftGrip?: boolean;
	sharedRightGrip?: boolean;
	displaySpan?: Span;
	embedded?: boolean;
	videoPath?: string | null;
	sourceSpan?: Span;
	id: string;
	span: Span;
	rowId: string;
	disabled?: boolean;
	children: React.ReactNode;
	isSelected?: boolean;
	onSelect?: () => void;
	onDoubleClick?: () => void;
	onSelectId?: (id: string) => void;
	zoomDepth?: number;
	zoomScale?: number;
	zoomMode?: "auto" | "manual";
	speedValue?: number;
	waveformPeaks?: AudioPeaksData | null;
	waveformSegmentSpan?: Span;
	waveformGain?: number;
	waveformNormalize?: boolean;
	muted?: boolean;
	variant?: "zoom" | "trim" | "clip" | "annotation" | "speed" | "audio" | "caption";
	isLoading?: boolean;
	loadingLabel?: string;
	annotationStages?: AnnotationStages;
	annotationType?: AnnotationType;
	annotationKeyframes?: { id: string; time: number; changes?: string[] }[];
	selectedKeyframeId?: string | null;
	onSelectKeyframe?: (id: string) => void;
	onMoveKeyframe?: (id: string, time: number) => void;
}

// Map zoom depth to multiplier labels
const ZOOM_LABELS: Record<number, string> = {
	1: "1.25×",
	2: "1.5×",
	3: "1.8×",
	4: "2.2×",
	5: "3.5×",
	6: "5×",
};

export default function Item({
	clipPresentation: suppliedPresentation,
	id,
	sharedLeftGrip = false,
	sharedRightGrip = false,
	embedded = false,
	videoPath,
	sourceSpan,
	span,
	displaySpan: suppliedDisplaySpan,
	rowId,
	disabled = false,
	isSelected = false,
	onSelect,
	onDoubleClick,
	onSelectId,
	zoomDepth = 1,
	zoomScale,
	zoomMode = "auto",
	speedValue,
	waveformPeaks = null,
	waveformSegmentSpan,
	waveformGain = 1,
	waveformNormalize = false,
	muted = false,
	variant = "zoom",
	isLoading = false,
	loadingLabel,
	annotationStages,
	annotationType,
	annotationKeyframes = [],
	selectedKeyframeId,
	onSelectKeyframe,
	onMoveKeyframe,
	children,
}: ItemProps) {
	const timeline = useTimelineContext();
	const presentation = useTimelinePresentation();
	const clipPresentation =
		variant === "clip" ? undefined : (suppliedPresentation ?? presentation.clips);
	const displaySpan = clipPresentation?.length
		? getRegionDisplaySpan(span, clipPresentation)
		: (suppliedDisplaySpan ?? span);
	const nodeRef = useRef<HTMLDivElement | null>(null);
	const [hoveredKeyframe, setHoveredKeyframe] = useState<{ id: string; x: number; y: number } | null>(null);
	const targets = useMemo(
		() => [
			...new Set(
				presentation.regions
					.filter((region) => region.id !== id)
					.flatMap((region) => [region.start, region.end]),
			),
		],
		[presentation.regions, id],
	);
	const snap = (next: Span, edge?: "start" | "end") =>
		snapRegionSpan(next, targets, presentation.clips, timeline.pixelsToValue(1), edge);
	const getMediaSpanFromDrag: GetSpanFromDragEvent = (event) => {
		if (!("delta" in event)) return span;
		const dragged = timeline.getSpanFromDragEvent(event);
		if (!dragged) return null;
		if (clipPresentation) {
			const start = getTimeAtClipSeam(dragged.start, clipPresentation);
			return snap({ start, end: start + span.end - span.start });
		}
		const visualOffset = displaySpan.start - span.start;
		return { start: dragged.start - visualOffset, end: dragged.end - visualOffset };
	};
	const getMediaSpanFromResize: GetSpanFromResizeEvent = (event) => {
		const delta = timeline.pixelsToValue(event.delta.x);
		const edge = event.direction;
		if (clipPresentation)
			return snap(
				{
					...span,
					[edge]: getTimeAtClipSeam(displaySpan[edge] + delta, clipPresentation),
				},
				edge,
			);
		const scale = (span.end - span.start) / (displaySpan.end - displaySpan.start);
		return { ...span, [edge]: span[edge] + delta * scale };
	};

	const paintPreview = (preview: Span, deltaY = 0) => {
		const node = nodeRef.current;
		if (!node || !clipPresentation) return;
		const display = getRegionDisplaySpan(preview, clipPresentation);
		const side = timeline.direction === "rtl" ? "right" : "left";
		node.style[side] = `${timeline.valueToPixels(display.start - timeline.range.start)}px`;
		node.style.width = `${timeline.valueToPixels(display.end - display.start)}px`;
		node.style.transform = deltaY ? `translateY(${deltaY}px)` : "none";
	};
	const { previewConfig } = presentation;
	useDndMonitor({
		onDragMove(event) {
			if (event.active.id !== id || !clipPresentation) return;
			const next = getMediaSpanFromDrag(event);
			if (next) {
				const resolved = resolveDragEnd(id, next, rowId, previewConfig);
				paintPreview(resolved?.span ?? span, event.delta.y);
			}
		},
		onDragEnd(event) {
			if (event.active.id === id) paintPreview(span);
		},
		onDragCancel(event) {
			if (event.active.id === id) paintPreview(span);
		},
	});
	const { setNodeRef, attributes, listeners, itemStyle, itemContentStyle } = useItem({
		id,
		span: displaySpan,
		disabled: disabled || isLoading,
		data: {
			rowId,
			span,
			getSpanFromDragEvent: getMediaSpanFromDrag,
			getSpanFromResizeEvent: getMediaSpanFromResize,
		},
		// Match the interaction area to the visible grip. The former inherited 28 px
		// zone made audio clips resize when pressing the body beside the white line.
		resizeHandleWidth: variant === "clip" ? 12 : variant === "audio" ? 10 : undefined,
		onResizeMove(event) {
			if (!clipPresentation) return;
			const next = getMediaSpanFromResize(event);
			if (next) {
				const resolved = resolveResizeEnd(id, next, previewConfig);
				if (resolved) paintPreview(resolved);
			}
		},
	});

	const attachRef = useCallback(
		(node: HTMLDivElement | null) => {
			nodeRef.current = node;
			setNodeRef(node);
		},
		[setNodeRef],
	);
	const timeLabel = useMemo(
		() => `${formatPlayheadTime(span.start)} – ${formatPlayheadTime(span.end)}`,
		[span.start, span.end],
	);

	if (isLoading) {
		return (
			<div
				ref={attachRef}
				style={{
					...itemStyle,
					height: "100%",
					display: "flex",
					alignItems: "center",
				}}
				{...listeners}
				{...attributes}
				data-timeline-item="true"
				data-variant={variant}
				data-start-ms={span.start}
				data-end-ms={span.end}
				onMouseDownCapture={(event) => event.stopPropagation()}
				onClickCapture={(event) => event.stopPropagation()}
			>
				<Skeleton
					animationType="shimmer"
					aria-label={loadingLabel || "Loading..."}
					className="w-full"
					style={{ height: "85%", minHeight: 22 }}
				/>
			</div>
		);
	}

	const isZoom = variant === "zoom";
	const isTrim = variant === "trim";
	const isClip = variant === "clip";
	const isSpeed = variant === "speed";
	const isAudio = variant === "audio";
	const isCaption = variant === "caption";
	const annotationDuration = Math.max(1, span.end - span.start);
	const inBoundaryPercent = annotationStages?.inEnabled
		? Math.min(100, (annotationStages.inDurationMs / annotationDuration) * 100)
		: 0;
	const outBoundaryPercent = annotationStages?.outEnabled
		? Math.max(0, 100 - (annotationStages.outDurationMs / annotationDuration) * 100)
		: 100;
	const showAudioWaveform = isAudio && Boolean(waveformPeaks);
	const clipSpeedLabel = isClip ? formatClipSpeedLabel(speedValue ?? 1) : null;
	const AnnotationIcon =
		annotationType === "text"
			? TextT
			: annotationType === "image"
				? ImageSquare
				: annotationType === "figure"
					? ArrowRight
					: annotationType === "blur"
						? BoundingBox
						: annotationType === "drawing"
							? PencilSimple
							: MessageSquare;

	const glassClass = isZoom
		? glassStyles.glassBlue
		: isTrim
			? glassStyles.glassRed
			: isClip
				? glassStyles.glassCyan
				: isSpeed
					? glassStyles.glassAmber
					: isAudio
						? glassStyles.glassDarkGreen
						: isCaption
							? glassStyles.glassCaption
							: glassStyles.glassYellow;

	const MIN_ITEM_PX = 6;
	const handleSelect = () => {
		onSelect?.();
		onSelectId?.(id);
	};
	const safeItemStyle = {
		...itemStyle,
		minWidth: MIN_ITEM_PX,
		height: "100%",
		overflow: isClip ? "visible" : "hidden",
		pointerEvents: "auto" as const,
	};

	return (
		<div
			ref={attachRef}
			style={safeItemStyle}
			{...listeners}
			{...attributes}
			data-timeline-item="true"
			data-variant={variant}
			data-start-ms={span.start}
			data-end-ms={span.end}
			aria-label={
				isClip
					? `Clip${clipSpeedLabel ? ` ${clipSpeedLabel}` : ""} · ${timeLabel}`
					: undefined
			}
			onPointerDownCapture={handleSelect}
			onDoubleClick={(event) => {
				if (!onDoubleClick) return;
				event.stopPropagation();
				onDoubleClick();
			}}
			className="group h-full"
		>
			<div
				className="h-full"
				style={{
					...itemContentStyle,
					overflow: isClip ? "visible" : "hidden",
					minWidth: MIN_ITEM_PX,
					height: "100%",
					display: "flex",
					alignItems: "center",
				}}
			>
				<div
					className={cn(
						glassClass,
						"timeline-block w-full flex items-center justify-center gap-1.5 relative",
						isSelected && glassStyles.selected,
						embedded && glassStyles.embeddedCaption,
						isClip || variant === "annotation" ? "overflow-visible" : "overflow-hidden",
					)}
					style={{
						height: "85%",
						minHeight: embedded ? 18 : 22,
						minWidth: MIN_ITEM_PX,
						containerType: "size",
					}}
					onClick={(event) => {
						event.stopPropagation();
					}}
				>
					{isClip && videoPath && (
						<ClipFilmstrip
							path={videoPath}
							span={span}
							sourceSpan={sourceSpan ?? span}
						/>
					)}
					<div
						className={cn(
							glassStyles.zoomEndCap,
							glassStyles.left,
							isClip && glassStyles.clipHandle,
							isAudio && glassStyles.audioHandle,
						)}
						style={{
							cursor: "col-resize",
							pointerEvents: isClip ? "none" : "auto",
							display: isClip && sharedLeftGrip ? "none" : undefined,
						}}
						title="Resize left"
					/>
					<div
						className={cn(
							glassStyles.zoomEndCap,
							glassStyles.right,
							isClip && glassStyles.clipHandle,
							isAudio && glassStyles.audioHandle,
						)}
						style={{
							cursor: "col-resize",
							pointerEvents: isClip ? "none" : "auto",
							display: isClip && sharedRightGrip ? "none" : undefined,
						}}
						title="Resize right"
					/>
					{showAudioWaveform && waveformPeaks && (
						<AudioWaveform
							peaks={waveformPeaks}
							segmentStartMs={waveformSegmentSpan?.start ?? span.start}
							segmentEndMs={waveformSegmentSpan?.end ?? span.end}
							gain={waveformGain}
							normalize={waveformNormalize}
							className="absolute inset-0 w-full h-full pointer-events-none opacity-45"
						/>
					)}
					{variant === "annotation" &&
						(annotationStages?.inEnabled || annotationStages?.outEnabled) && (
							<div
								className="pointer-events-none absolute inset-y-0 left-0 right-0 z-20"
								aria-label="Annotation stages"
							>
								{annotationStages.inEnabled && (
									<div
										className="absolute inset-y-0 border-r border-white/80 border-dashed"
										style={{ left: `${inBoundaryPercent}%` }}
										title={`In: ${Math.round(annotationStages.inDurationMs)} ms`}
									/>
								)}
								{annotationStages.outEnabled && (
									<div
										className="absolute inset-y-0 border-l border-white/80 border-dashed"
										style={{ left: `${outBoundaryPercent}%` }}
										title={`Out: ${Math.round(annotationStages.outDurationMs)} ms`}
									/>
								)}
							</div>
						)}
					{variant === "annotation" && annotationKeyframes.map((keyframe) => {
						const percent = Math.max(0, Math.min(100, ((keyframe.time - span.start) / Math.max(1, span.end - span.start)) * 100));
						return (
							<button
								key={keyframe.id}
								type="button"
								className="group/keyframe absolute z-30 h-3 w-3 -translate-x-1/2 cursor-grab active:cursor-grabbing"
								style={{ left: `${percent}%`, top: "50%", marginTop: "-6px", opacity: keyframe.id === selectedKeyframeId ? 1 : 0.75 }}
								onPointerDown={(event) => {
									event.stopPropagation();
									if (!isSelected) {
										onSelectId?.(id);
										onSelectKeyframe?.(keyframe.id);
										return;
									}
									onSelectKeyframe?.(keyframe.id);
									const block = event.currentTarget.parentElement;
									if (!block) return;
									const move = (moveEvent: PointerEvent) => {
										const rect = block.getBoundingClientRect();
										const progress = Math.max(0, Math.min(1, (moveEvent.clientX - rect.left) / Math.max(1, rect.width)));
										onMoveKeyframe?.(keyframe.id, span.start + progress * (span.end - span.start));
									};
									const up = () => {
										window.removeEventListener("pointermove", move);
										window.removeEventListener("pointerup", up);
									};
									window.addEventListener("pointermove", move);
									window.addEventListener("pointerup", up);
								}}
								onClick={(event) => event.stopPropagation()}
								onMouseEnter={(event) => {
									const rect = event.currentTarget.getBoundingClientRect();
									setHoveredKeyframe({ id: keyframe.id, x: rect.left + rect.width / 2, y: rect.top });
								}}
								onMouseLeave={() => setHoveredKeyframe(null)}
								onFocus={(event) => {
									const rect = event.currentTarget.getBoundingClientRect();
									setHoveredKeyframe({ id: keyframe.id, x: rect.left + rect.width / 2, y: rect.top });
								}}
								onBlur={() => setHoveredKeyframe(null)}
								aria-label={`Keyframe @ ${Math.round(keyframe.time)}ms (drag to move)`}
							>
								<span className="block h-3 w-3 rotate-45 border border-black/60 bg-cyan-300 shadow-[0_0_6px_rgba(34,211,238,.9)]" />
							</button>
						);
					})}
					{/* Muted overlay for source audio track items */}
					{isAudio && muted && (
						<div className="absolute inset-0 z-20 flex items-center justify-center gap-1 bg-red-900/40 pointer-events-none">
							<SpeakerX className="w-3 h-3 text-red-300/90 shrink-0" />
						</div>
					)}
					{/* Normal-speed clips show only the filmstrip and resize handles. */}
					{(!isClip || clipSpeedLabel) && (
						<div
							title={`${isZoom ? `${zoomScale?.toFixed(2) ?? ZOOM_LABELS[zoomDepth]}${zoomScale === undefined ? "" : "×"} ${zoomMode === "manual" ? "Manual" : "Auto"}` : typeof children === "string" ? children : "Clip"} · ${timeLabel}`}
							className={cn(
								"relative z-10 flex max-w-full items-center justify-center gap-1 px-1 text-[11px] font-medium text-black/70 dark:text-white/90 select-none overflow-hidden",
								isClip &&
									"rounded bg-black/65 px-2 py-1 text-white dark:text-white",
								(isZoom || embedded) && "text-white dark:text-white",
								embedded && "w-full justify-start px-2",
							)}
						>
							{isClip ? (
								clipSpeedLabel
							) : isZoom ? (
								<>
									<ZoomIn
										aria-hidden="true"
										className="zoom-icon size-3 shrink-0"
									/>
									<span className="zoom-value whitespace-nowrap">
										{zoomScale === undefined ? ZOOM_LABELS[zoomDepth] || `${zoomDepth}×` : `${zoomScale.toFixed(2)}×`}
										<span className="zoom-mode font-normal">
											{zoomMode === "manual" ? "Manual" : "Auto"}
										</span>
									</span>
								</>
							) : embedded ? (
								<span className="truncate">{children}</span>
							) : variant === "annotation" ? (
								<>
									<AnnotationIcon className="size-3 shrink-0" />
									<span className="truncate">{children}</span>
								</>
							) : (
								<>
									{isTrim ? (
										<Scissors className="size-3 shrink-0" />
									) : isSpeed ? (
										<Gauge className="size-3 shrink-0" />
									) : isAudio ? (
										<Music className="size-3 shrink-0" />
									) : (
										<MessageSquare className="size-3 shrink-0" />
									)}
									<span className="truncate">
										{isTrim ? "Trim" : isSpeed ? `${speedValue}×` : children}
									</span>
								</>
							)}
						</div>
					)}
				</div>
			</div>
			{hoveredKeyframe && (() => {
				const frame = annotationKeyframes.find((item) => item.id === hoveredKeyframe.id);
				if (!frame) return null;
				const width = Math.min(320, window.innerWidth - 16);
				const below = hoveredKeyframe.y < 180;
				const availableHeight = below ? window.innerHeight - hoveredKeyframe.y - 30 : hoveredKeyframe.y - 20;
				return createPortal(
					<div role="tooltip" className="pointer-events-none fixed z-[9999] overflow-y-auto rounded-md border border-foreground/20 bg-editor-bg px-2.5 py-2 text-left text-[11px] text-foreground shadow-xl" style={{ width, left: Math.max(8, Math.min(window.innerWidth - width - 8, hoveredKeyframe.x - width / 2)), maxHeight: Math.max(40, Math.min(320, availableHeight)), ...(below ? { top: hoveredKeyframe.y + 22 } : { bottom: window.innerHeight - hoveredKeyframe.y + 10 }) }}>
						<strong className="block pb-1">Keyframe {Math.round(frame.time)} ms</strong>
						{(frame.changes ?? []).map((change, index) => <span key={`${index}-${change}`} className="block break-words">{change}</span>)}
					</div>,
					document.body,
				);
			})()}
		</div>
	);
}
