import { useEffect, useRef, useState, type RefObject } from "react";
import type { useVideoEditorAudio } from "../audio/useVideoEditorAudio";
import { retimeCaptionFragment } from "../captionTimeline";
import type { useAnnotationRegionCommands } from "../hooks/useAnnotationRegionCommands";
import type { useAudioRegionCommands } from "../hooks/useAudioRegionCommands";
import type { useCaptionCommands } from "../hooks/useCaptionCommands";
import type { useClipRegionCommands } from "../hooks/useClipRegionCommands";
import type { useEditorPlaybackControls } from "../hooks/useEditorPlaybackControls";
import type { useTimelineProjection } from "../hooks/useTimelineProjection";
import type { useZoomRegionCommands } from "../hooks/useZoomRegionCommands";
import type { useTimelineState } from "../state/useTimelineState";
import TimelineEditor, { type TimelineEditorHandle } from "../timeline/TimelineEditor";
import {
	type TimelineWindowAction,
	type TimelineWindowMessage,
	type TimelineWindowSnapshot,
} from "../timeline/timelineWindowProtocol";

type Props = {
	panelRef?: RefObject<HTMLDivElement | null>;
	timelineRef: RefObject<TimelineEditorHandle | null>;
	timeline: ReturnType<typeof useTimelineState>;
	projection: ReturnType<typeof useTimelineProjection>;
	playback: ReturnType<typeof useEditorPlaybackControls>;
	audio: ReturnType<typeof useVideoEditorAudio>;
	zoomCommands: ReturnType<typeof useZoomRegionCommands>;
	clipCommands: ReturnType<typeof useClipRegionCommands>;
	audioCommands: ReturnType<typeof useAudioRegionCommands>;
	captionCommands: ReturnType<typeof useCaptionCommands>;
	annotationCommands: ReturnType<typeof useAnnotationRegionCommands>;
	videoPath: string | null;
	videoSourcePath: string | null;
	cursorTelemetrySourcePath: string | null;
	normalizedCursorTelemetry: ReturnType<typeof useTimelineState>["cursorTelemetry"];
	autoSuggestZoomsTrigger: number;
	handleAutoSuggestZoomsConsumed: () => void;
	disableSuggestedZooms: boolean;
	currentTime: number;
	isPlaying: boolean;
	captionGenerationProgress?: number;
	handleSelectAnnotation: (id: string | null) => void;
};

export function EditorTimelinePanel(props: Props) {
	const timelineWindowClipboard = useRef<
		| { kind: "annotation"; region: ReturnType<typeof useTimelineState>["annotationRegions"][number] }
		| { kind: "zoom"; region: ReturnType<typeof useTimelineState>["zoomRegions"][number] }
		| null
	>(null);
	const [timelineWindowOpen, setTimelineWindowOpen] = useState(false);
	const [panelHeight, setPanelHeight] = useState<number | null>(null);
	const [rowScale, setRowScale] = useState(1);
	const resizeStart = useRef<{ y: number; height: number; max: number } | null>(null);
	const {
		timelineRef,
		timeline,
		projection,
		playback,
		audio,
		zoomCommands,
		clipCommands,
		audioCommands,
		captionCommands,
		annotationCommands,
		videoPath,
		videoSourcePath,
		cursorTelemetrySourcePath,
		normalizedCursorTelemetry,
		autoSuggestZoomsTrigger,
		handleAutoSuggestZoomsConsumed,
		disableSuggestedZooms,
		currentTime,
		captionGenerationProgress = 0,
		handleSelectAnnotation,
	} = props;

	const selectedCaptionId =
		projection.effectiveCaptionRegions.find(
			(cue) =>
				cue.sourceCueId === timeline.selectedCaptionId &&
				currentTime * 1000 >= cue.startMs &&
				currentTime * 1000 < cue.endMs,
		)?.id ??
		projection.effectiveCaptionRegions.find(
			(cue) => cue.sourceCueId === timeline.selectedCaptionId,
		)?.id ??
		null;

	const snapshot: TimelineWindowSnapshot = {
		isPlaying: props.isPlaying,
		rowScale,
		videoDuration: projection.timelineDuration,
		currentTime,
		playheadTime: projection.timelinePlayheadTime,
		videoPath,
		videoSourcePath,
		cursorTelemetrySourcePath,
		cursorTelemetry: normalizedCursorTelemetry,
		autoSuggestZoomsTrigger,
		disableSuggestedZooms,
		zoomRegions: timeline.zoomRegions,
		selectedZoomId: timeline.selectedZoomId,
		trimRegions: timeline.trimRegions,
		clipRegions: timeline.clipRegions,
		selectedClipId: timeline.selectedClipId,
		audioRegions: timeline.audioRegions,
		selectedAudioId: timeline.selectedAudioId,
		captionRegions: projection.effectiveCaptionRegions,
		selectedCaptionId,
		captionsEnabled: timeline.autoCaptionSettings.enabled,
		captionQuickAddEnabled: timeline.autoCaptionSettings.timelineQuickAdd,
		annotationRegions: timeline.annotationRegions,
		selectedAnnotationId: timeline.selectedAnnotationId,
		showSourceAudioTrack: timeline.clipRegions.some((clip) => clip.showSourceAudio),
		sourceAudioResourceVersion: timeline.sourceAudioFallbackRefreshKey,
		sourceAudioTrackSettings: audio.activeSourceAudioTrackSettings,
	};

	useEffect(() => {
		const sendSnapshot = () => {
			window.electronAPI.sendTimelineWindowMessage({ type: "snapshot", snapshot });
		};
		const handleAction = (action: TimelineWindowAction) => {
			switch (action.type) {
				case "clipboard": {
					if (action.operation === "paste") {
						if (timelineWindowClipboard.current?.kind === "annotation") annotationCommands.handleAnnotationDuplicate(timelineWindowClipboard.current.region);
						if (timelineWindowClipboard.current?.kind === "zoom") zoomCommands.handleZoomDuplicate(timelineWindowClipboard.current.region);
						break;
					}
					const annotation = timeline.selectedAnnotationId ? timeline.annotationRegions.find((region) => region.id === timeline.selectedAnnotationId) : undefined;
					const zoom = timeline.selectedZoomId ? timeline.zoomRegions.find((region) => region.id === timeline.selectedZoomId) : undefined;
					if (annotation) {
						timelineWindowClipboard.current = { kind: "annotation", region: structuredClone(annotation) };
						if (action.operation === "cut") annotationCommands.handleAnnotationDelete(annotation.id);
					} else if (zoom) {
						timelineWindowClipboard.current = { kind: "zoom", region: structuredClone(zoom) };
						if (action.operation === "cut") zoomCommands.handleZoomDelete(zoom.id);
					}
					break;
				}
				case "toggle-play": playback.togglePlayPause(); break;
				case "skip-back": playback.handlePreviewSkipBack(); break;
				case "skip-forward": playback.handlePreviewSkipForward(); break;
				case "row-scale": setRowScale(Math.max(0.5, Math.min(3, action.value))); break;
				case "seek": playback.handleTimelineSeek(action.time); break;
				case "auto-suggest-consumed": handleAutoSuggestZoomsConsumed(); break;
				case "zoom-added": zoomCommands.handleZoomAdded(action.span); break;
				case "zoom-suggested": zoomCommands.handleZoomSuggested(action.span, action.focus); break;
				case "zoom-span-change": zoomCommands.handleZoomSpanChange(action.id, action.span); break;
				case "zoom-delete": zoomCommands.handleZoomDelete(action.id); break;
				case "select-zoom": zoomCommands.handleSelectZoom(action.id); break;
				case "clip-split": clipCommands.handleClipSplit(action.splitMs); break;
				case "clip-span-change": clipCommands.handleClipSpanChange(action.id, action.span); break;
				case "clip-delete": clipCommands.handleClipDelete(action.id); break;
				case "select-clip": clipCommands.handleSelectClip(action.id); break;
				case "audio-added": audioCommands.handleAudioAdded(action.span, action.audioPath, action.trackIndex); break;
				case "audio-span-change": audioCommands.handleAudioSpanChange(action.id, action.span, action.trackIndex); break;
				case "audio-delete": audioCommands.handleAudioDelete(action.id); break;
				case "select-audio": audioCommands.handleSelectAudio(action.id); break;
				case "caption-span-change": {
					const fragment = projection.effectiveCaptionRegions.find((cue) => cue.id === action.id);
					if (fragment) captionCommands.handleCaptionRetime(fragment.sourceCueId, retimeCaptionFragment(fragment, action.span));
					break;
				}
				case "caption-delete": {
					const fragment = projection.effectiveCaptionRegions.find((cue) => cue.id === action.id);
					if (fragment) captionCommands.handleCaptionDelete(fragment.sourceCueId);
					break;
				}
				case "caption-added": captionCommands.handleCaptionAdded(action.span); break;
				case "select-caption": {
					const fragment = projection.effectiveCaptionRegions.find((cue) => cue.id === action.id);
					captionCommands.handleSelectCaption(fragment?.sourceCueId ?? null);
					if (fragment) playback.handleTimelineSeek(fragment.startMs / 1000);
					break;
				}
				case "annotation-added": annotationCommands.handleAnnotationAdded(action.span, action.trackIndex); break;
				case "annotation-span-change": annotationCommands.handleAnnotationSpanChange(action.id, action.span, action.trackIndex); break;
				case "annotation-delete": annotationCommands.handleAnnotationDelete(action.id); break;
				case "select-annotation": handleSelectAnnotation(action.id); break;
			}
		};
		const unsubscribe = window.electronAPI.onTimelineWindowMessage((message) => {
			const event = message as TimelineWindowMessage;
			if (event.type === "snapshot-request") sendSnapshot();
			if (event.type === "action") handleAction(event.action);
			if (event.type === "window-closed") setTimelineWindowOpen(false);
			if (event.type === "window-ready") setTimelineWindowOpen(true);
		});
		sendSnapshot();
		return () => {
			unsubscribe();
		};
	}, [snapshot]);

	const timelineEditor = (
		<TimelineEditor
				rowScale={rowScale}
				onRowScaleChange={setRowScale}
				ref={timelineRef}
				videoDuration={projection.timelineDuration}
				currentTime={currentTime}
				playheadTime={projection.timelinePlayheadTime}
				onSeek={playback.handleTimelineSeek}
				videoPath={videoPath}
				videoSourcePath={videoSourcePath}
				cursorTelemetrySourcePath={cursorTelemetrySourcePath}
				cursorTelemetry={normalizedCursorTelemetry}
				autoSuggestZoomsTrigger={autoSuggestZoomsTrigger}
				onAutoSuggestZoomsConsumed={handleAutoSuggestZoomsConsumed}
				disableSuggestedZooms={disableSuggestedZooms}
				zoomRegions={timeline.zoomRegions}
				onZoomAdded={zoomCommands.handleZoomAdded}
				onZoomSuggested={zoomCommands.handleZoomSuggested}
				onZoomSpanChange={zoomCommands.handleZoomSpanChange}
				onZoomDelete={zoomCommands.handleZoomDelete}
				selectedZoomId={timeline.selectedZoomId}
				onSelectZoom={zoomCommands.handleSelectZoom}
				trimRegions={timeline.trimRegions}
				clipRegions={timeline.clipRegions}
				onClipSplit={clipCommands.handleClipSplit}
				onClipDelete={clipCommands.handleClipDelete}
				onClipSpanChange={clipCommands.handleClipSpanChange}
				selectedClipId={timeline.selectedClipId}
				onSelectClip={clipCommands.handleSelectClip}
				audioRegions={timeline.audioRegions}
				onAudioAdded={audioCommands.handleAudioAdded}
				onAudioSpanChange={audioCommands.handleAudioSpanChange}
				onAudioDelete={audioCommands.handleAudioDelete}
				selectedAudioId={timeline.selectedAudioId}
				onSelectAudio={audioCommands.handleSelectAudio}
				captionRegions={projection.effectiveCaptionRegions}
				onCaptionSpanChange={(id, span) => {
					const fragment = projection.effectiveCaptionRegions.find(
						(cue) => cue.id === id,
					);
					if (!fragment) return;
					captionCommands.handleCaptionRetime(
						fragment.sourceCueId,
						retimeCaptionFragment(fragment, span),
					);
				}}
				selectedCaptionId={selectedCaptionId}
				onSelectCaption={(id) => {
					const fragment = projection.effectiveCaptionRegions.find(
						(cue) => cue.id === id,
					);
					captionCommands.handleSelectCaption(fragment?.sourceCueId ?? null);
					if (fragment) playback.handleTimelineSeek(fragment.startMs / 1000);
				}}
				onCaptionDelete={(id) => {
					const fragment = projection.effectiveCaptionRegions.find(
						(cue) => cue.id === id,
					);
					if (fragment) captionCommands.handleCaptionDelete(fragment.sourceCueId);
				}}
				onCaptionAdded={captionCommands.handleCaptionAdded}
				captionsEnabled={timeline.autoCaptionSettings.enabled}
				captionQuickAddEnabled={timeline.autoCaptionSettings.timelineQuickAdd}
				annotationRegions={timeline.annotationRegions}
				onAnnotationAdded={annotationCommands.handleAnnotationAdded}
				onAnnotationSpanChange={annotationCommands.handleAnnotationSpanChange}
				onAnnotationDelete={annotationCommands.handleAnnotationDelete}
				selectedAnnotationId={timeline.selectedAnnotationId}
				onSelectAnnotation={handleSelectAnnotation}
				showSourceAudioTrack={timeline.clipRegions.some((clip) => clip.showSourceAudio)}
				sourceAudioResourceVersion={timeline.sourceAudioFallbackRefreshKey}
				sourceAudioTrackSettings={audio.activeSourceAudioTrackSettings}
				getSourceAudioTrackSettingsForClip={audio.getSourceAudioTrackSettingsForClip}
				onSourceAudioAvailabilityChange={timeline.setHasClipSourceAudio}
				onSourceAudioTracksMetaChange={audio.onSourceAudioTracksMetaChange}
			/>
	);

	return (
		<div
				ref={props.panelRef}
				tabIndex={-1}
				data-timeline-panel
				className="outline-none relative flex flex-shrink-0 flex-col bg-transparent px-4 pb-4 pt-2"
				style={{ height: panelHeight ?? "22%", minHeight: 140, maxHeight: "75%", display: timelineWindowOpen ? "none" : undefined }}
			>
				<div
					role="separator" aria-label="Resize timeline" aria-orientation="horizontal"
					tabIndex={0} title="Drag to resize timeline"
					className="absolute inset-x-0 top-0 z-30 h-2 cursor-row-resize touch-none border-t border-separator hover:bg-primary/25 focus:bg-primary/25"
					onPointerDown={(event) => {
						if (event.button !== 0) return;
						const panel = event.currentTarget.parentElement!;
						resizeStart.current = { y: event.clientY, height: panel.getBoundingClientRect().height, max: (panel.parentElement?.clientHeight ?? window.innerHeight) * 0.75 };
						event.currentTarget.setPointerCapture(event.pointerId);
						event.preventDefault();
					}}
					onPointerMove={(event) => {
						const start = resizeStart.current;
						if (start) setPanelHeight(Math.max(140, Math.min(start.max, start.height + start.y - event.clientY)));
					}}
					onPointerUp={() => { resizeStart.current = null; }}
					onLostPointerCapture={() => { resizeStart.current = null; }}
					onKeyDown={(event) => {
						if (event.key !== "ArrowUp" && event.key !== "ArrowDown") return;
						event.preventDefault();
						const panel = event.currentTarget.parentElement!;
						setPanelHeight(Math.max(140, Math.min((panel.parentElement?.clientHeight ?? window.innerHeight) * 0.75, panel.clientHeight + (event.key === "ArrowUp" ? 24 : -24))));
					}}
				/>
				<div className="flex flex-shrink-0 items-center justify-end pb-2">
					<button
						type="button"
						className="flex items-center gap-1.5 rounded-md border border-primary/50 bg-primary/15 px-2.5 py-1.5 text-xs font-medium text-primary shadow-sm transition-colors hover:bg-primary hover:text-primary-foreground"
						onClick={() => {
							void window.electronAPI.openTimelineWindow();
						}}
						title="Open the timeline in a separate desktop window"
					>
						<span aria-hidden="true">↗</span>
						Open Timeline Window
					</button>
				</div>
				<div className="relative">
					{timelineEditor}
					{captionGenerationProgress > 0 ? (
						<div className="pointer-events-none absolute left-0 right-0 top-0 z-40 h-1 overflow-hidden bg-primary/15" role="progressbar" aria-label="Generating captions" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(captionGenerationProgress)}>
							<div className="h-full bg-primary transition-[width] duration-500" style={{ width: `${captionGenerationProgress}%` }} />
						</div>
					) : null}
				</div>
			</div>
	);
}
