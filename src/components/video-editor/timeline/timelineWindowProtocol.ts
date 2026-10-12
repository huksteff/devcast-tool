import type { Span } from "dnd-timeline";
import type { SourceAudioTrackSettings } from "../audio/audioTypes";
import type {
	AnnotationRegion,
	AnnotationPositionKeyframe,
	KeyframeEditMode,
	AudioRegion,
	CaptionCue,
	ClipRegion,
	CursorTelemetryPoint,
	TrimRegion,
	ZoomFocus,
	ZoomRegion,
} from "../types";
import type { ClipSequenceSpan } from "./core/timelineTypes";

export const TIMELINE_WINDOW_CHANNEL = "devcast-timeline-window";

export type TimelineWindowSnapshot = {
	isPlaying: boolean;
	keyframeMode: KeyframeEditMode;
	showKeyframes: boolean;
	selectedKeyframeId: string | null;
	canGenerateAutoCaptions: boolean;
	isGeneratingCaptions: boolean;
	rowScale: number;
	videoDuration: number;
	currentTime: number;
	playheadTime: number;
	videoPath: string | null;
	videoSourcePath: string | null;
	cursorTelemetrySourcePath: string | null;
	cursorTelemetry: CursorTelemetryPoint[];
	autoSuggestZoomsTrigger: number;
	disableSuggestedZooms: boolean;
	zoomRegions: ZoomRegion[];
	selectedZoomId: string | null;
	trimRegions: TrimRegion[];
	clipRegions: ClipRegion[];
	selectedClipId: string | null;
	audioRegions: AudioRegion[];
	audioTrackCount: number;
	selectedAudioId: string | null;
	captionRegions: CaptionCue[];
	selectedCaptionId: string | null;
	captionsEnabled: boolean;
	captionQuickAddEnabled: boolean;
	annotationRegions: AnnotationRegion[];
	selectedAnnotationId: string | null;
	showSourceAudioTrack: boolean;
	sourceAudioResourceVersion: number;
	sourceAudioTrackSettings: SourceAudioTrackSettings;
};

export type TimelineWindowAction =
	| { type: "toggle-play" }
	| { type: "skip-back" }
	| { type: "skip-forward" }
	| { type: "row-scale"; value: number }
	| { type: "toggle-keyframe-mode" }
	| { type: "set-keyframe-mode"; mode: KeyframeEditMode }
	| { type: "toggle-keyframe-visibility" }
	| { type: "select-keyframe"; id: string | null }
	| { type: "copy-keyframe" }
	| { type: "paste-keyframe" }
	| { type: "generate-captions" }
	| { type: "seek"; time: number }
	| { type: "auto-suggest-consumed" }
	| { type: "zoom-added"; span: Span }
	| { type: "zoom-suggested"; span: Span; focus: ZoomFocus }
	| { type: "zoom-span-change"; id: string; span: Span }
	| { type: "zoom-delete"; id: string }
	| { type: "select-zoom"; id: string | null }
	| { type: "clip-split"; splitMs: number }
	| { type: "clip-span-change"; id: string; span: ClipSequenceSpan }
	| { type: "clip-delete"; id: string }
	| { type: "select-clip"; id: string | null }
	| { type: "audio-added"; span: Span; audioPath: string; trackIndex?: number }
	| { type: "audio-span-change"; id: string; span: Span; trackIndex?: number }
	| { type: "audio-delete"; id: string }
	| { type: "audio-track-added" }
	| { type: "select-audio"; id: string | null }
	| { type: "caption-span-change"; id: string; span: Span }
	| { type: "caption-delete"; id: string }
	| { type: "caption-added"; span: Span }
	| { type: "select-caption"; id: string | null }
	| { type: "annotation-added"; span: Span; trackIndex?: number }
	| { type: "annotation-span-change"; id: string; span: Span; trackIndex?: number }
	| { type: "annotation-delete"; id: string }
	| { type: "annotation-position-keyframes-change"; id: string; keyframes: AnnotationPositionKeyframe[] }
	| { type: "select-annotation"; id: string | null }
	| { type: "clipboard"; operation: "copy" | "cut" | "paste" };

export type TimelineWindowMessage =
	| { type: "snapshot-request" }
	| { type: "snapshot"; snapshot: TimelineWindowSnapshot }
	| { type: "action"; action: TimelineWindowAction }
	| { type: "window-closed" }
	| { type: "window-ready" };
