import { useEffect, useState } from "react";
import TimelineEditor from "./TimelineEditor";
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

	if (!snapshot) {
		return (
			<div className="flex h-screen items-center justify-center bg-editor-bg text-sm text-muted-foreground">
				Waiting for the editor timeline…
			</div>
		);
	}

	return (
		<div className="flex h-screen min-h-0 flex-col bg-editor-bg p-3 text-foreground">
			<div className="mb-2 flex flex-shrink-0 items-center justify-between border-b border-separator pb-2">
				<span className="text-sm font-medium">Timeline</span>
				<button
					type="button"
					className="rounded-md border border-separator px-2.5 py-1 text-xs text-muted-foreground hover:bg-default hover:text-foreground"
					onClick={() => void window.electronAPI.closeTimelineWindow()}
				>
					Close window
				</button>
			</div>
			<div className="flex min-h-0 flex-1 flex-col">
				<TimelineEditor
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
					onCaptionSpanChange={(id, span) => send({ type: "caption-span-change", id, span })}
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
