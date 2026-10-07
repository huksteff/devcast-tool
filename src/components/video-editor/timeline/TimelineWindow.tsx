import { useEffect, useState } from "react";
import TimelineEditor from "./TimelineEditor";
import { Play, Pause, SkipBack, SkipForward } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import {
	type TimelineWindowAction,
	type TimelineWindowMessage,
	type TimelineWindowSnapshot,
} from "./timelineWindowProtocol";

export default function TimelineWindow() {
	const [snapshot, setSnapshot] = useState<TimelineWindowSnapshot | null>(null);

	useEffect(() => {
		const unsubscribe = window.electronAPI.onTimelineWindowMessage((message) => {
			const event = message as TimelineWindowMessage;
			if (event.type === "snapshot") setSnapshot(event.snapshot);
		});
		window.electronAPI.sendTimelineWindowMessage({ type: "snapshot-request" });
		return () => {
			unsubscribe();
		};
	}, []);

	const send = (action: TimelineWindowAction) => {
		window.electronAPI.sendTimelineWindowMessage({ type: "action", action });
	};
	useEffect(() => {
		if (snapshot) window.electronAPI.sendTimelineWindowMessage({ type: "window-ready" });
	}, [Boolean(snapshot)]);
	useEffect(() => {
		const handleKeyDown = (event: KeyboardEvent) => {
			if (!(event.ctrlKey || event.metaKey) || event.altKey || event.isComposing) return;
			const target = event.target as HTMLElement | null;
			if (target?.matches("input, textarea, select") || target?.isContentEditable) return;
			const key = event.key.toLowerCase();
			if (key !== "c" && key !== "x" && key !== "v") return;
			event.preventDefault();
			send({
				type: "clipboard",
				operation: key === "c" ? "copy" : key === "x" ? "cut" : "paste",
			});
		};
		window.addEventListener("keydown", handleKeyDown);
		return () => window.removeEventListener("keydown", handleKeyDown);
	}, []);

	if (!snapshot) {
		return (
			<div className="flex h-screen items-center justify-center bg-editor-bg text-sm text-muted-foreground">
				Waiting for the editor timeline…
			</div>
		);
	}

	return (
		<div className="flex h-screen min-h-0 flex-col bg-editor-bg p-3 text-foreground">
			<div className="mb-2 grid flex-shrink-0 grid-cols-[1fr_auto_1fr] items-center border-b border-separator pb-2">
				<span className="text-sm font-medium">Timeline</span>
				<div className="flex items-center gap-1.5">
					<span className="text-xs tabular-nums text-muted-foreground">
						{Math.floor(snapshot.playheadTime / 60)}:
						{String(Math.floor(snapshot.playheadTime % 60)).padStart(2, "0")}
					</span>
					<Button
						variant="ghost"
						size="icon"
						title="Skip back"
						aria-label="Skip back"
						onClick={() => send({ type: "skip-back" })}
					>
						<SkipBack weight="fill" />
					</Button>
					<Button
						variant="ghost"
						size="icon"
						className="rounded-full bg-foreground text-background"
						title={snapshot.isPlaying ? "Pause" : "Play"}
						aria-label={snapshot.isPlaying ? "Pause" : "Play"}
						onClick={() => send({ type: "toggle-play" })}
					>
						{snapshot.isPlaying ? <Pause weight="fill" /> : <Play weight="fill" />}
					</Button>
					<Button
						variant="ghost"
						size="icon"
						title="Skip forward"
						aria-label="Skip forward"
						onClick={() => send({ type: "skip-forward" })}
					>
						<SkipForward weight="fill" />
					</Button>
					<span className="text-xs tabular-nums text-muted-foreground">
						{Math.floor(snapshot.videoDuration / 60)}:
						{String(Math.floor(snapshot.videoDuration % 60)).padStart(2, "0")}
					</span>
				</div>
				<div aria-hidden="true" />
			</div>
			<div className="flex min-h-0 flex-1 flex-col">
				<TimelineEditor
					rowScale={snapshot.rowScale}
					onRowScaleChange={(value) => send({ type: "row-scale", value })}
					videoDuration={snapshot.videoDuration}
					currentTime={snapshot.currentTime}
					playheadTime={snapshot.playheadTime}
					onSeek={(time) => send({ type: "seek", time })}
					videoPath={snapshot.videoPath}
					videoSourcePath={snapshot.videoSourcePath}
					cursorTelemetrySourcePath={snapshot.cursorTelemetrySourcePath}
					cursorTelemetry={snapshot.cursorTelemetry}
					autoSuggestZoomsTrigger={0}
					onAutoSuggestZoomsConsumed={() => send({ type: "auto-suggest-consumed" })}
					disableSuggestedZooms={snapshot.disableSuggestedZooms}
					zoomRegions={snapshot.zoomRegions}
					onZoomAdded={(span) => send({ type: "zoom-added", span })}
					onZoomSuggested={(span, focus) => send({ type: "zoom-suggested", span, focus })}
					onZoomSpanChange={(id, span) => send({ type: "zoom-span-change", id, span })}
					onZoomDelete={(id) => send({ type: "zoom-delete", id })}
					selectedZoomId={snapshot.selectedZoomId}
					onSelectZoom={(id) => send({ type: "select-zoom", id })}
					trimRegions={snapshot.trimRegions}
					clipRegions={snapshot.clipRegions}
					onClipSplit={(splitMs) => send({ type: "clip-split", splitMs })}
					onClipSpanChange={(id, span) => send({ type: "clip-span-change", id, span })}
					onClipDelete={(id) => send({ type: "clip-delete", id })}
					selectedClipId={snapshot.selectedClipId}
					onSelectClip={(id) => send({ type: "select-clip", id })}
					audioRegions={snapshot.audioRegions}
					onAudioAdded={(span, audioPath, trackIndex) =>
						send({ type: "audio-added", span, audioPath, trackIndex })
					}
					onAudioSpanChange={(id, span, trackIndex) =>
						send({ type: "audio-span-change", id, span, trackIndex })
					}
					onAudioDelete={(id) => send({ type: "audio-delete", id })}
					selectedAudioId={snapshot.selectedAudioId}
					onSelectAudio={(id) => send({ type: "select-audio", id })}
					captionRegions={snapshot.captionRegions}
					onCaptionSpanChange={(id, span) =>
						send({ type: "caption-span-change", id, span })
					}
					onCaptionDelete={(id) => send({ type: "caption-delete", id })}
					onCaptionAdded={(span) => send({ type: "caption-added", span })}
					captionsEnabled={snapshot.captionsEnabled}
					captionQuickAddEnabled={snapshot.captionQuickAddEnabled}
					selectedCaptionId={snapshot.selectedCaptionId}
					onSelectCaption={(id) => send({ type: "select-caption", id })}
					annotationRegions={snapshot.annotationRegions}
					onAnnotationAdded={(span, trackIndex) =>
						send({ type: "annotation-added", span, trackIndex })
					}
					onAnnotationSpanChange={(id, span, trackIndex) =>
						send({ type: "annotation-span-change", id, span, trackIndex })
					}
					onAnnotationDelete={(id) => send({ type: "annotation-delete", id })}
					selectedAnnotationId={snapshot.selectedAnnotationId}
					onSelectAnnotation={(id) => send({ type: "select-annotation", id })}
					showSourceAudioTrack={snapshot.showSourceAudioTrack}
					sourceAudioResourceVersion={snapshot.sourceAudioResourceVersion}
					sourceAudioTrackSettings={snapshot.sourceAudioTrackSettings}
					getSourceAudioTrackSettingsForClip={() => snapshot.sourceAudioTrackSettings}
				/>
			</div>
		</div>
	);
}
