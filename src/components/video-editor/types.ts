export type ZoomDepth = 1 | 2 | 3 | 4 | 5 | 6;

export interface ZoomFocus {
	cx: number; // normalized horizontal center (0-1)
	cy: number; // normalized vertical center (0-1)
}

export type ZoomMode = "auto" | "manual";

export interface ZoomRegion {
	id: string;
	startMs: number;
	endMs: number;
	depth: ZoomDepth;
	focus: ZoomFocus;
	mode?: ZoomMode;
}

export interface CursorTelemetryPoint {
	timeMs: number;
	cx: number;
	cy: number;
	pressure?: number;
	interactionType?:
		| "move"
		| "click"
		| "double-click"
		| "right-click"
		| "middle-click"
		| "mouseup";
	cursorType?:
		| "arrow"
		| "text"
		| "pointer"
		| "crosshair"
		| "open-hand"
		| "closed-hand"
		| "resize-ew"
		| "resize-ns"
		| "not-allowed";
}

export interface CursorVisualSettings {
	size: number;
	smoothing: number;
	motionBlur: number;
	clickBounce: number;
	clickBounceDuration: number;
	clickEffect: CursorClickEffectStyle;
	clickEffectColor: string;
	clickEffectScale: number;
	clickEffectOpacity: number;
	clickEffectDurationMs: number;
	sway: number;
	style: CursorStyle;
	calloutEnabled: boolean;
	calloutText: string;
	calloutBackground: string;
	calloutTextColor: string;
	calloutScale: number;
}

export type CursorStyle =
	| "macos"
	| "tahoe"
	| "tahoe-inverted"
	| "windows11"
	| "dot"
	| "figma"
	| (string & {});
export const DEFAULT_CURSOR_STYLE: CursorStyle = "tahoe";

export type CursorClickEffectStyle = "none" | "spotlight" | "ripple" | "echo";
export const DEFAULT_CURSOR_CLICK_EFFECT: CursorClickEffectStyle = "none";
export const DEFAULT_CURSOR_CLICK_EFFECT_COLOR = "#2563EB";
export const DEFAULT_CURSOR_CLICK_EFFECT_SCALE = 1;
export const DEFAULT_CURSOR_CLICK_EFFECT_OPACITY = 1;
export const DEFAULT_CURSOR_CLICK_EFFECT_DURATION_MS = 600;
export const DEFAULT_CURSOR_CALLOUT_TEXT = "";
export const DEFAULT_CURSOR_CALLOUT_BACKGROUND = "#14B8C4";
export const DEFAULT_CURSOR_CALLOUT_TEXT_COLOR = "#FFFFFF";
export const DEFAULT_CURSOR_CALLOUT_SCALE = 1;

export function normalizeCursorClickEffectStyle(
	value: unknown,
	fallback: CursorClickEffectStyle = DEFAULT_CURSOR_CLICK_EFFECT,
): CursorClickEffectStyle {
	if (value === "burst") {
		return "echo";
	}

	return value === "none" || value === "spotlight" || value === "ripple" || value === "echo"
		? value
		: fallback;
}

export function normalizeCursorClickEffectColor(
	value: unknown,
	fallback: string = DEFAULT_CURSOR_CLICK_EFFECT_COLOR,
): string {
	if (typeof value !== "string") {
		return fallback;
	}

	const trimmed = value.trim();
	if (!/^#(?:[0-9a-fA-F]{3}|[0-9a-fA-F]{6})$/.test(trimmed)) {
		return fallback;
	}

	if (trimmed.length === 4) {
		const [red, green, blue] = trimmed.slice(1).split("");
		return `#${red}${red}${green}${green}${blue}${blue}`.toUpperCase();
	}

	return trimmed.toUpperCase();
}

export type EditorEffectSection =
	| "scene"
	| "cursor"
	| "captions"
	| "caption"
	| "webcam"
	| "settings"
	| "zoom"
	| "frame"
	| "crop"
	| "extensions"
	| "clip"
	| "audio"
	| "audio"
	| `ext:${string}`;

export type ZoomTransitionEasing = "devcast" | "glide" | "smooth" | "snappy" | "linear";

export type WebcamCorner = "top-left" | "top-right" | "bottom-left" | "bottom-right";
export type WebcamPositionPreset =
	| WebcamCorner
	| "top-center"
	| "center-left"
	| "center"
	| "center-right"
	| "bottom-center"
	| "custom";

export interface WebcamOverlaySettings {
	/** Source-time intervals containing webcam footage in an imported sequence. */
	visibleRanges?: { startMs: number; endMs: number }[];
	enabled: boolean;
	sourcePath: string | null;
	timeOffsetMs: number;
	mirror: boolean;
	cropRegion: CropRegion;
	corner: WebcamCorner;
	positionPreset: WebcamPositionPreset;
	positionX: number;
	positionY: number;
	size: number;
	width: number;
	height: number;
	reactToZoom: boolean;
	roundness: number;
	/** Legacy pixel value retained only while loading older projects. */
	cornerRadius?: number;
	shadow: number;
	margin: number;
}

export const DEFAULT_CURSOR_SIZE = 3.0;
export const DEFAULT_CURSOR_SMOOTHING = 0.67;
export const DEFAULT_CURSOR_MOTION_BLUR = 0.6;
export const DEFAULT_CURSOR_CLICK_BOUNCE = 2;
export const DEFAULT_CURSOR_CLICK_BOUNCE_DURATION = 350;
export const DEFAULT_CURSOR_SWAY = 0.4;
export const DEFAULT_ZOOM_SMOOTHNESS = 0.5;
export const DEFAULT_ZOOM_MOTION_BLUR = 0.35;
export interface ZoomMotionBlurTuning {
	panVelocityThreshold: number;
	zoomVelocityThreshold: number;
	maxDirectionalBlurPx: number;
	maxRadialBlurStrength: number;
	panResponsePerSecond: number;
	zoomResponsePerSecond: number;
	zoomSafeZoneRadiusPx: number;
}

export const DEFAULT_ZOOM_MOTION_BLUR_TUNING: ZoomMotionBlurTuning = {
	panVelocityThreshold: 0,
	zoomVelocityThreshold: 0,
	maxDirectionalBlurPx: 41.8,
	maxRadialBlurStrength: 1,
	panResponsePerSecond: 11,
	zoomResponsePerSecond: 9,
	zoomSafeZoneRadiusPx: 6,
};
export const DEFAULT_ZOOM_IN_DURATION_MS = 1522.575;
export const DEFAULT_ZOOM_IN_OVERLAP_MS = 500;
export const DEFAULT_ZOOM_OUT_DURATION_MS = 1015.05;
export const DEFAULT_CONNECTED_ZOOM_GAP_MS = 1500;
export const DEFAULT_CONNECTED_ZOOM_DURATION_MS = 1000;
export const DEFAULT_ZOOM_IN_EASING: ZoomTransitionEasing = "devcast";
export const DEFAULT_ZOOM_OUT_EASING: ZoomTransitionEasing = "devcast";
export const DEFAULT_CONNECTED_ZOOM_EASING: ZoomTransitionEasing = "glide";
export const DEFAULT_WEBCAM_SIZE = 40;
export const DEFAULT_WEBCAM_REACT_TO_ZOOM = true;
export const DEFAULT_WEBCAM_ROUNDNESS = 100;
export const DEFAULT_WEBCAM_SHADOW = 0.3;
export const DEFAULT_WEBCAM_MARGIN = 24;
export const DEFAULT_WEBCAM_POSITION_PRESET: WebcamPositionPreset = "bottom-right";
export const DEFAULT_WEBCAM_POSITION_X = 1;
export const DEFAULT_WEBCAM_POSITION_Y = 1;
export const DEFAULT_WEBCAM_TIME_OFFSET_MS = 0;

export const DEFAULT_WEBCAM_OVERLAY: WebcamOverlaySettings = {
	enabled: false,
	sourcePath: null,
	timeOffsetMs: DEFAULT_WEBCAM_TIME_OFFSET_MS,
	mirror: true,
	cropRegion: { x: 0, y: 0, width: 1, height: 1 },
	corner: "bottom-right",
	positionPreset: DEFAULT_WEBCAM_POSITION_PRESET,
	positionX: DEFAULT_WEBCAM_POSITION_X,
	positionY: DEFAULT_WEBCAM_POSITION_Y,
	size: DEFAULT_WEBCAM_SIZE,
	width: DEFAULT_WEBCAM_SIZE,
	height: DEFAULT_WEBCAM_SIZE,
	reactToZoom: DEFAULT_WEBCAM_REACT_TO_ZOOM,
	roundness: DEFAULT_WEBCAM_ROUNDNESS,
	shadow: DEFAULT_WEBCAM_SHADOW,
	margin: DEFAULT_WEBCAM_MARGIN,
};

export interface TrimRegion {
	id: string;
	startMs: number;
	endMs: number;
}

export interface ClipRegion {
	id: string;
	/** Where the clip sits on the timeline. */
	startMs: number;
	/** Where the clip ends on the timeline (`startMs` + source duration / speed). */
	endMs: number;
	/**
	 * Where the clip reads from in the recording. Defaults to `startMs`, which is
	 * only the same thing while everything before it plays at 1x — splitting or
	 * left-trimming a sped-up clip moves the source in without moving the clip.
	 */
	sourceStartMs?: number;
	/** Bounds of this recording within the shared media source. */
	sourceMinMs?: number;
	sourceMaxMs?: number;
	speed: number;
	muted?: boolean;
	showSourceAudio?: boolean;
}

export function getClipSourceStartMs(clip: ClipRegion): number {
	return Number.isFinite(clip.sourceStartMs) ? (clip.sourceStartMs as number) : clip.startMs;
}

export function getClipSourceEndMs(clip: ClipRegion): number {
	const displayDurationMs = Math.max(0, clip.endMs - clip.startMs);
	const speed = Number.isFinite(clip.speed) && clip.speed > 0 ? clip.speed : 1;
	return Math.round(getClipSourceStartMs(clip) + displayDurationMs * speed);
}

export function getTimelineDurationMs(clips: ClipRegion[], sourceDurationMs: number): number {
	const baseDurationMs = Math.max(0, Math.round(sourceDurationMs));
	if (clips.length === 0) {
		return baseDurationMs;
	}

	return clips.reduce(
		(durationMs, clip) => Math.max(durationMs, Math.max(0, Math.round(clip.endMs))),
		0,
	);
}

export function sortClipRegions(clips: ClipRegion[]): ClipRegion[] {
	return [...clips].sort((left, right) => left.startMs - right.startMs);
}

function getSafeClipSpeed(clip: ClipRegion) {
	return Number.isFinite(clip.speed) && clip.speed > 0 ? clip.speed : 1;
}

function mapNearestClipBoundary(timeMs: number, clips: ClipRegion[], from: "timeline" | "source") {
	let nearestTimeMs = Math.round(timeMs);
	let nearestDistance = Number.POSITIVE_INFINITY;

	for (const clip of clips) {
		const boundaries = [
			[clip.startMs, getClipSourceStartMs(clip)],
			[clip.endMs, getClipSourceEndMs(clip)],
		];

		for (const [timelineTimeMs, sourceTimeMs] of boundaries) {
			const inputTimeMs = from === "timeline" ? timelineTimeMs : sourceTimeMs;
			const distance = Math.abs(timeMs - inputTimeMs);
			if (distance < nearestDistance) {
				nearestDistance = distance;
				nearestTimeMs = Math.round(from === "timeline" ? sourceTimeMs : timelineTimeMs);
			}
		}
	}

	return nearestTimeMs;
}

export function mapTimelineTimeToSourceTime(timeMs: number, clips: ClipRegion[]): number {
	const roundedTimeMs = Math.round(timeMs);
	const sortedClips = sortClipRegions(clips);

	for (const clip of sortedClips) {
		if (timeMs < clip.startMs || timeMs >= clip.endMs) {
			continue;
		}

		return Math.round(
			getClipSourceStartMs(clip) + (roundedTimeMs - clip.startMs) * getSafeClipSpeed(clip),
		);
	}

	if (sortedClips.length === 0) {
		return roundedTimeMs;
	}

	return mapNearestClipBoundary(roundedTimeMs, sortedClips, "timeline");
}

export function mapSourceTimeToTimelineTime(timeMs: number, clips: ClipRegion[]): number {
	const roundedTimeMs = Math.round(timeMs);
	const sortedClips = sortClipRegions(clips);

	for (const clip of sortedClips) {
		const sourceStartMs = getClipSourceStartMs(clip);
		const sourceEndMs = getClipSourceEndMs(clip);
		if (timeMs < sourceStartMs || timeMs >= sourceEndMs) {
			continue;
		}

		return Math.round(clip.startMs + (roundedTimeMs - sourceStartMs) / getSafeClipSpeed(clip));
	}

	if (sortedClips.length === 0) {
		return roundedTimeMs;
	}

	return mapNearestClipBoundary(roundedTimeMs, sortedClips, "source");
}

export function findClipAtTimelineTime(timeMs: number, clips: ClipRegion[]): ClipRegion | null {
	return (
		sortClipRegions(clips).find((clip) => timeMs >= clip.startMs && timeMs < clip.endMs) ?? null
	);
}

export function extendAutoFullTrackClip(
	clips: ClipRegion[],
	autoClipId: string | null,
	previousAutoEndMs: number | null,
	nextTotalDurationMs: number,
): ClipRegion[] | null {
	if (
		!autoClipId ||
		!Number.isFinite(previousAutoEndMs) ||
		!Number.isFinite(nextTotalDurationMs) ||
		nextTotalDurationMs <= (previousAutoEndMs ?? 0) ||
		clips.length !== 1
	) {
		return null;
	}

	const [clip] = clips;
	if (
		clip.id !== autoClipId ||
		clip.startMs !== 0 ||
		clip.speed !== 1 ||
		clip.endMs !== previousAutoEndMs
	) {
		return null;
	}

	return [{ ...clip, endMs: nextTotalDurationMs }];
}

/** Convert clip regions (kept segments) to trim regions (gaps to remove). */
export function clipsToTrims(clips: ClipRegion[], totalDurationMs: number): TrimRegion[] {
	if (clips.length === 0) return [];
	// Clips are ordered on the timeline, but a moved clip keeps its source
	// in-point, so timeline order says nothing about source order. Walk the
	// source ranges the clips claim and trim whatever is left uncovered.
	const coveredSpans = clips
		.map((clip) => ({
			startMs: getClipSourceStartMs(clip),
			endMs: getClipSourceEndMs(clip),
		}))
		.filter((span) => span.endMs > span.startMs)
		.sort((left, right) => left.startMs - right.startMs);
	const trims: TrimRegion[] = [];
	let cursor = 0;
	let trimId = 1;
	for (const span of coveredSpans) {
		if (span.startMs > cursor) {
			trims.push({
				id: `trim-gap-${trimId++}`,
				startMs: cursor,
				endMs: span.startMs,
			});
		}
		cursor = Math.max(cursor, span.endMs);
	}
	if (cursor < totalDurationMs) {
		trims.push({
			id: `trim-gap-${trimId++}`,
			startMs: cursor,
			endMs: totalDurationMs,
		});
	}
	return trims;
}

/** Convert legacy trim regions to clip regions (complement). */
export function trimsToClips(trims: TrimRegion[], totalDurationMs: number): ClipRegion[] {
	if (trims.length === 0) return [{ id: "clip-1", startMs: 0, endMs: totalDurationMs, speed: 1 }];
	const sorted = [...trims].sort((a, b) => a.startMs - b.startMs);
	const clips: ClipRegion[] = [];
	let cursor = 0;
	let clipId = 1;
	for (const trim of sorted) {
		if (trim.startMs > cursor) {
			clips.push({
				id: `clip-${clipId++}`,
				startMs: cursor,
				endMs: trim.startMs,
				speed: 1,
			});
		}
		cursor = trim.endMs;
	}
	if (cursor < totalDurationMs) {
		clips.push({
			id: `clip-${clipId++}`,
			startMs: cursor,
			endMs: totalDurationMs,
			speed: 1,
		});
	}
	return clips;
}

/** Visual layers placed over the recording. `drawing` stores a freehand stroke. */
export type AnnotationType = "text" | "image" | "figure" | "blur" | "drawing" | "click" | "cursor-callout";
export const BLUR_ANNOTATION_STRENGTH = 20;
export const BASE_PREVIEW_WIDTH = 1920;
export const BASE_PREVIEW_HEIGHT = 1080;

export type ArrowDirection =
	| "up"
	| "down"
	| "left"
	| "right"
	| "up-right"
	| "up-left"
	| "down-right"
	| "down-left";

export interface FigureData {
	kind?: "arrow" | "rectangle" | "ellipse" | "circle" | "line" | "highlight";
	arrowDirection: ArrowDirection;
	color: string;
	strokeWidth: number;
	/** A left-to-right animated, glowing multi-colour stroke. */
	gradient?: AnnotationGradient;
}

export interface AnnotationGradient {
	enabled: boolean;
	/** Any number of colours can be saved; the editor exposes the first four. */
	colors: string[];
	cycleDurationMs: number;
	glow: number;
}

export const DEFAULT_ANNOTATION_GRADIENT: AnnotationGradient = {
	enabled: false,
	// Keep the initial editor state deliberately simple. Extra stops are opt-in
	// through the 2–4 colour control, instead of silently changing old presets.
	colors: ["#FF4D9D", "#22D3EE"],
	cycleDurationMs: 2400,
	glow: 14,
};

export const DEFAULT_CURSOR_CALLOUT_GRADIENT = {
	enabled: false,
	colors: ["#14B8C4", "#A855F7"],
	glow: 14,
} as const;

export interface ClickHighlightData {
	color: string;
	particleCount: number;
	radius: number;
	durationMs: number;
}

/** A zero-content timeline event controlling the cursor label. */
export interface CursorCalloutEventData {
	action: "enable" | "disable" | "text";
}

export interface ResolvedCursorCalloutState {
	enabled: boolean;
	text: string;
}

/**
 * Applies callout events in timeline order. The event's span is its typewriter
 * duration: text writes during the span; disable erases during the span and
 * only hides after it completes. This shared resolver keeps preview and export
 * frame-perfect.
 */
export function resolveCursorCalloutState(
	annotations: AnnotationRegion[] | undefined,
	timeMs: number,
	fallback: ResolvedCursorCalloutState,
): ResolvedCursorCalloutState {
	let state = { ...fallback };
	const events = (annotations ?? [])
		.filter((annotation) => annotation.type === "cursor-callout" && annotation.startMs <= timeMs)
		.sort((left, right) => left.startMs - right.startMs || left.id.localeCompare(right.id));
	for (const event of events) {
		const action = event.calloutEvent?.action ?? "text";
		const duration = Math.max(1, event.endMs - event.startMs);
		const progress = Math.max(0, Math.min(1, (timeMs - event.startMs) / duration));
		if (action === "enable") {
			state = { enabled: true, text: "" };
		} else if (action === "text" && state.enabled) {
			const target = event.content.slice(0, 80);
			state.text = target.slice(0, Math.round(target.length * progress));
			if (progress >= 1) state.text = target;
		} else if (action === "disable" && state.enabled) {
			state.text = state.text.slice(0, Math.round(state.text.length * (1 - progress)));
			if (progress >= 1) state = { enabled: false, text: "" };
		}
	}
	return state;
}

export const DEFAULT_CLICK_HIGHLIGHT_DATA: ClickHighlightData = {
	color: "#60A5FA",
	particleCount: 18,
	radius: 90,
	durationMs: 750,
};

export interface DrawingPoint {
	x: number;
	y: number;
}

export interface DrawingData {
	/**
	 * Legacy flattened stroke. Kept so projects made before multi-stroke drawing
	 * continue to load without a migration.
	 */
	points: DrawingPoint[];
	/** Each pointer gesture is a separate path, so separate marks never join. */
	strokes?: DrawingPoint[][];
	color: string;
	strokeWidth: number;
	/** Active editor tool. Persisted so the overlay and settings panel stay in sync. */
	tool?: "pencil" | "eraser";
	eraserSize?: number;
	/** Show the eraser head while the Draw Out stage retraces the path. */
	showEraserInDrawOut?: boolean;
	gradient?: AnnotationGradient;
}

/**
 * `draw-in` and `draw-out` are intentionally drawing-only transitions. They
 * reveal the actual path rather than fading its finished result, which makes
 * a hand-drawn annotation feel like it is being made (or erased) on screen.
 */
export type AnnotationTransition =
	| "none"
	| "fade"
	| "pop"
	| "slide-up"
	| "slide-down"
	| "slide-left"
	| "slide-right"
	| "bounce"
	| "zoom"
	| "swing"
	| "typewriter"
	| "draw-in"
	| "draw-out";
export interface AnnotationStages {
	inEnabled: boolean;
	inDurationMs: number;
	inTransition: AnnotationTransition;
	outEnabled: boolean;
	outDurationMs: number;
	outTransition: AnnotationTransition;
}
export const DEFAULT_ANNOTATION_STAGES: AnnotationStages = {
	inEnabled: false,
	inDurationMs: 300,
	inTransition: "fade",
	outEnabled: false,
	outDurationMs: 300,
	outTransition: "fade",
};

/** Presentation transform for an annotation at a timeline timestamp.
 * Kept in the shared editor model so preview and export follow the same rules. */
export function getAnnotationStageState(
	annotation: Pick<AnnotationRegion, "startMs" | "endMs" | "stages">,
	timeMs: number,
) {
	const stages = annotation.stages;
	if (!stages)
		return {
			alpha: 1,
			translateX: 0,
			translateY: 0,
			scale: 1,
			rotate: 0,
			drawingProgress: 1,
			textProgress: 1,
		};
	let progress: number | null = null;
	let transition = stages.inTransition;
	let entering = true;
	if (stages.inEnabled && timeMs < annotation.startMs + stages.inDurationMs) {
		progress = Math.max(
			0,
			Math.min(1, (timeMs - annotation.startMs) / Math.max(1, stages.inDurationMs)),
		);
	} else if (stages.outEnabled && timeMs > annotation.endMs - stages.outDurationMs) {
		progress = Math.max(
			0,
			Math.min(1, (annotation.endMs - timeMs) / Math.max(1, stages.outDurationMs)),
		);
		transition = stages.outTransition;
		entering = false;
	}
	if (progress === null || transition === "none") {
		return {
			alpha: 1,
			translateX: 0,
			translateY: 0,
			scale: 1,
			rotate: 0,
			drawingProgress: 1,
			textProgress: 1,
		};
	}
	if (transition === "draw-in" || transition === "draw-out") {
		// During an Out stage progress runs from 1 to 0, naturally erasing the
		// path from its finishing end. During In it runs 0 to 1 and draws it.
		return {
			alpha: 1,
			translateX: 0,
			translateY: 0,
			scale: 1,
			rotate: 0,
			drawingProgress: progress,
			textProgress: 1,
		};
	}
	if (transition === "typewriter")
		return {
			alpha: 1,
			translateX: 0,
			translateY: 0,
			scale: 1,
			rotate: 0,
			drawingProgress: 1,
			textProgress: progress,
		};
	// Use a smooth progress curve for transform-based transitions. Unlike a
	// CSS transition this is evaluated from the media clock, so preview and
	// every export frame follow exactly the same motion.
	const eased = progress * progress * (3 - 2 * progress);
	if (transition === "bounce") {
		const bounce = Math.sin(eased * Math.PI * 3) * (1 - eased) * 0.38;
		return {
			alpha: eased,
			translateX: 0,
			translateY: 0,
			scale: 0.52 + eased * 0.48 + bounce,
			rotate: 0,
			drawingProgress: 1,
			textProgress: 1,
		};
	}
	if (transition === "zoom")
		return {
			alpha: eased,
			translateX: 0,
			translateY: 0,
			scale: 0.12 + eased * 0.88,
			rotate: 0,
			drawingProgress: 1,
			textProgress: 1,
		};
	if (transition === "swing")
		return {
			alpha: eased,
			translateX: 0,
			translateY: 0,
			scale: 1,
			rotate:
				Math.sin(eased * Math.PI * 3.25) * (1 - eased) * (entering ? -36 : 36),
			drawingProgress: 1,
			textProgress: 1,
		};
	if (transition === "pop")
		return {
			alpha: eased,
			translateX: 0,
			translateY: 0,
			scale: 0.58 + eased * 0.42 + Math.sin(eased * Math.PI) * 0.14,
			rotate: 0,
			drawingProgress: 1,
			textProgress: 1,
		};
	if (transition === "slide-up")
		return {
			alpha: eased,
			translateX: 0,
			translateY: (1 - eased) * (entering ? 72 : -72),
			scale: 1,
			rotate: 0,
			drawingProgress: 1,
			textProgress: 1,
		};
	if (transition === "slide-down")
		return {
			alpha: eased,
			translateX: 0,
			translateY: (1 - eased) * (entering ? -72 : 72),
			scale: 1,
			rotate: 0,
			drawingProgress: 1,
			textProgress: 1,
		};
	if (transition === "slide-left" || transition === "slide-right") {
		const direction = transition === "slide-left" ? -1 : 1;
		return {
			alpha: eased,
			translateX: (1 - eased) * direction * (entering ? 96 : -96),
			translateY: 0,
			scale: 1,
			rotate: 0,
			drawingProgress: 1,
			textProgress: 1,
		};
	}
	return {
		alpha: progress,
		translateX: 0,
		translateY: 0,
		scale: 1,
		rotate: 0,
		drawingProgress: 1,
		textProgress: 1,
	};
}

/** Return the portion of a multi-stroke drawing visible at `progress`.
 * The progress is distributed by actual path length, rather than point count,
 * so long segments draw at a consistent speed. Shared by preview and export. */
export function getVisibleDrawingStrokes(
	drawing: DrawingData | undefined,
	progress: number,
): DrawingPoint[][] {
	if (!drawing) return [];
	const strokes = drawing.strokes?.length
		? drawing.strokes
		: drawing.points?.length
			? [drawing.points]
			: [];
	const boundedProgress = Math.max(0, Math.min(1, progress));
	if (boundedProgress >= 1) return strokes.map((stroke) => [...stroke]);
	if (boundedProgress <= 0) return [];

	const lengths = strokes.map((stroke) => {
		if (stroke.length <= 1) return stroke.length ? 1 : 0;
		let length = 0;
		for (let index = 1; index < stroke.length; index++) {
			length += Math.hypot(
				stroke[index].x - stroke[index - 1].x,
				stroke[index].y - stroke[index - 1].y,
			);
		}
		return Math.max(1, length);
	});
	let remaining = lengths.reduce((total, length) => total + length, 0) * boundedProgress;
	return strokes.map((stroke, strokeIndex) => {
		const strokeLength = lengths[strokeIndex];
		if (!stroke.length || remaining <= 0) return [];
		if (remaining >= strokeLength) {
			remaining -= strokeLength;
			return [...stroke];
		}
		if (stroke.length === 1) {
			remaining = 0;
			return [stroke[0]];
		}
		const visible = [stroke[0]];
		for (let index = 1; index < stroke.length && remaining > 0; index++) {
			const from = stroke[index - 1];
			const to = stroke[index];
			const segmentLength = Math.hypot(to.x - from.x, to.y - from.y);
			if (segmentLength === 0) continue;
			if (remaining >= segmentLength) {
				visible.push(to);
				remaining -= segmentLength;
			} else {
				const ratio = remaining / segmentLength;
				visible.push({
					x: from.x + (to.x - from.x) * ratio,
					y: from.y + (to.y - from.y) * ratio,
				});
				remaining = 0;
			}
		}
		return visible;
	});
}

export interface AnnotationPosition {
	x: number;
	y: number;
}

export interface AnnotationSize {
	width: number;
	height: number;
}

export interface AnnotationPositionKeyframe {
	id: string;
	timeMs: number;
	position: AnnotationPosition;
	/** Optional for projects saved before transform keyframes supported size. */
	size?: AnnotationSize;
}

export interface AnnotationTextStyle {
	color: string;
	backgroundColor: string;
	fontSize: number; // pixels
	fontFamily: string;
	fontWeight: "normal" | "bold";
	fontStyle: "normal" | "italic";
	textDecoration: "none" | "underline";
	textAlign: "left" | "center" | "right";
	borderRadius: number;
}

function getDefaultAnnotationFontFamily() {
	return '"SF Pro Display", "SF Pro Text", "Helvetica Neue", sans-serif';
}

export function getDefaultCaptionFontFamily() {
	return '"SF Pro Text", "SF Pro Display", "Helvetica Neue", sans-serif';
}

export interface AnnotationRegion {
	id: string;
	startMs: number;
	endMs: number;
	type: AnnotationType;
	content: string; // Legacy - still used for current type
	textContent?: string; // Separate storage for text
	imageContent?: string; // Separate storage for image data URL
	position: AnnotationPosition;
	positionKeyframes?: AnnotationPositionKeyframe[];
	size: AnnotationSize;
	style: AnnotationTextStyle;
	zIndex: number;
	trackIndex?: number;
	figureData?: FigureData;
	drawingData?: DrawingData;
	clickData?: ClickHighlightData;
	calloutEvent?: CursorCalloutEventData;
	stages?: AnnotationStages;
	blurIntensity?: number;
	blurColor?: string;
}

export const DEFAULT_ANNOTATION_POSITION: AnnotationPosition = {
	x: 50,
	y: 50,
};

export const DEFAULT_ANNOTATION_SIZE: AnnotationSize = {
	width: 30,
	height: 20,
};

export const DEFAULT_ANNOTATION_STYLE: AnnotationTextStyle = {
	color: "#ffffff",
	backgroundColor: "transparent",
	fontSize: 32,
	fontFamily: getDefaultAnnotationFontFamily(),
	fontWeight: "bold",
	fontStyle: "normal",
	textDecoration: "none",
	textAlign: "center",
	borderRadius: 8,
};

export const DEFAULT_FIGURE_DATA: FigureData = {
	kind: "arrow",
	arrowDirection: "right",
	color: "#2563EB",
	strokeWidth: 4,
};

export const DEFAULT_DRAWING_DATA: DrawingData = {
	points: [],
	color: "#2563EB",
	strokeWidth: 4,
	tool: "pencil",
	eraserSize: 12,
};

/**
 * Colour stops for the shared neon style. The offset advances through the
 * supplied colours, but the gradient direction is always left-to-right.
 */
export function getAnimatedGradientColors(
	gradient: AnnotationGradient | undefined,
	timeMs: number,
	fallback: string,
): string[] {
	if (!gradient?.enabled) return [fallback];
	const colors = gradient.colors.filter((color) => /^#[0-9a-f]{6}$/i.test(color));
	if (colors.length < 2) return [fallback];
	const duration = Math.max(
		250,
		gradient.cycleDurationMs || DEFAULT_ANNOTATION_GRADIENT.cycleDurationMs,
	);
	const offset = Math.floor(
		((((timeMs % duration) + duration) % duration) / duration) * colors.length,
	);
	return [...colors.slice(offset), ...colors.slice(0, offset)];
}

/** Erase a circular area while preserving the remaining stroke fragments. */
export function eraseDrawingAtPoint(
	drawing: DrawingData,
	point: DrawingPoint,
	radius: number,
): DrawingData {
	const source = drawing.strokes?.length
		? drawing.strokes
		: drawing.points?.length
			? [drawing.points]
			: [];
	const safeRadius = Math.max(0.1, radius);
	const fragments: DrawingPoint[][] = [];
	for (const stroke of source) {
		let fragment: DrawingPoint[] = [];
		for (const candidate of stroke) {
			if (Math.hypot(candidate.x - point.x, candidate.y - point.y) <= safeRadius) {
				if (fragment.length) fragments.push(fragment);
				fragment = [];
			} else {
				fragment.push(candidate);
			}
		}
		if (fragment.length) fragments.push(fragment);
	}
	return { ...drawing, points: fragments.flat(), strokes: fragments };
}

export interface CropRegion {
	x: number;
	y: number;
	width: number;
	height: number;
}

export const DEFAULT_CROP_REGION: CropRegion = {
	x: 0,
	y: 0,
	width: 1,
	height: 1,
};

export const ADVANCED_VERTICAL_PADDING_MAX = 250;

export interface Padding {
	top: number;
	bottom: number;
	left: number;
	right: number;
	linked?: boolean;
}

export const DEFAULT_PADDING: Padding = {
	top: 20,
	bottom: 20,
	left: 20,
	right: 20,
	linked: true,
};
export type {
	SourceAudioTrackSetting,
	SourceAudioTrackSettings,
} from "@/components/video-editor/audio/audioTypes";

export interface AudioRegion {
	id: string;
	startMs: number;
	endMs: number;
	audioPath: string;
	/** Base fader: 0–100%. */
	volume: number;
	/** Post-fader amplification, up to 10×. */
	gain?: number;
	normalize?: boolean;
	trackIndex?: number;
}

export interface CaptionCue {
	id: string;
	startMs: number;
	endMs: number;
	text: string;
	words?: CaptionCueWord[];
}

export interface CaptionCueWord {
	text: string;
	startMs: number;
	endMs: number;
	leadingSpace?: boolean;
}

export type AutoCaptionAnimation = "none" | "fade" | "rise" | "pop";

export interface AutoCaptionSettings {
	enabled: boolean;
	/** Show the hover ghost on the timeline caption track for click-to-add. */
	timelineQuickAdd: boolean;
	language: string;
	fontFamily: string;
	fontSize: number;
	bottomOffset: number;
	maxWidth: number;
	maxRows: number;
	animationStyle: AutoCaptionAnimation;
	boxRadius: number;
	textColor: string;
	inactiveTextColor: string;
	backgroundOpacity: number;
	textShadow: boolean;
}

export const DEFAULT_AUTO_CAPTION_SETTINGS: AutoCaptionSettings = {
	enabled: false,
	timelineQuickAdd: true,
	language: "auto",
	fontFamily: getDefaultCaptionFontFamily(),
	fontSize: 30,
	bottomOffset: 3,
	maxWidth: 62,
	maxRows: 1,
	animationStyle: "fade",
	boxRadius: 17.5,
	textColor: "#FFFFFF",
	inactiveTextColor: "#A3A3A3",
	backgroundOpacity: 0.9,
	textShadow: true,
};

export type PlaybackSpeed = 0.25 | 0.5 | 0.75 | 1.25 | 1.5 | 1.75 | 2;

export interface SpeedRegion {
	id: string;
	startMs: number;
	endMs: number;
	speed: PlaybackSpeed;
}

export const SPEED_OPTIONS: Array<{ speed: PlaybackSpeed; label: string }> = [
	{ speed: 0.25, label: "0.25×" },
	{ speed: 0.5, label: "0.5×" },
	{ speed: 0.75, label: "0.75×" },
	{ speed: 1.25, label: "1.25×" },
	{ speed: 1.5, label: "1.5×" },
	{ speed: 1.75, label: "1.75×" },
	{ speed: 2, label: "2×" },
];

export const DEFAULT_PLAYBACK_SPEED: PlaybackSpeed = 1.5;

export const ZOOM_DEPTH_SCALES: Record<ZoomDepth, number> = {
	1: 1.25,
	2: 1.5,
	3: 1.8,
	4: 2.2,
	5: 3.5,
	6: 5.0,
};

export const DEFAULT_ZOOM_DEPTH: ZoomDepth = 3;
export const DEFAULT_AUTO_ZOOM_DEPTH: ZoomDepth = 2;

export function clampFocusToDepth(focus: ZoomFocus, _depth: ZoomDepth): ZoomFocus {
	return {
		cx: clamp(focus.cx, 0, 1),
		cy: clamp(focus.cy, 0, 1),
	};
}

function clamp(value: number, min: number, max: number) {
	if (Number.isNaN(value)) return (min + max) / 2;
	return Math.min(max, Math.max(min, value));
}
