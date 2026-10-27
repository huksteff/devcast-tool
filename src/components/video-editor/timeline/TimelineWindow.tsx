import { useCallback, useEffect, useRef, useState } from "react";
import TimelineEditor, { type TimelineEditorHandle } from "./TimelineEditor";
import {
	CaretDown,
	Diamond,
	Eye,
	EyeSlash,
	MagicWand,
	MagnifyingGlassPlus,
	Play,
	Pause,
	Plus,
	Scissors,
	SkipBack,
	SkipForward,
	TextT,
} from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import {
	DropdownMenu,
	DropdownMenuContent,
	DropdownMenuItem,
	DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useI18n } from "@/contexts/I18nContext";
import {
	type TimelineWindowAction,
	type TimelineWindowMessage,
	type TimelineWindowSnapshot,
} from "./timelineWindowProtocol";

export default function TimelineWindow() {
	const [snapshot, setSnapshot] = useState<TimelineWindowSnapshot | null>(null);
	const timelineRef = useRef<TimelineEditorHandle | null>(null);
	const copiedKeyframeRef = useRef(false);
	const { t } = useI18n();

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

	const send = useCallback((action: TimelineWindowAction) => {
		window.electronAPI.sendTimelineWindowMessage({ type: "action", action });
	}, []);
	const hasSnapshot = snapshot !== null;
	useEffect(() => {
		if (hasSnapshot) window.electronAPI.sendTimelineWindowMessage({ type: "window-ready" });
	}, [hasSnapshot]);
	useEffect(() => {
		const handleKeyDown = (event: KeyboardEvent) => {
			if (!(event.ctrlKey || event.metaKey) || event.altKey || event.isComposing) return;
			const target = event.target as HTMLElement | null;
			if (target?.matches("input, textarea, select") || target?.isContentEditable) return;
			const key = event.key.toLowerCase();
			if (key !== "c" && key !== "x" && key !== "v") return;
			event.preventDefault();
			if (key === "c" && snapshot?.selectedKeyframeId) {
				copiedKeyframeRef.current = true;
				send({ type: "copy-keyframe" });
				return;
			}
			if (key === "v" && (copiedKeyframeRef.current || snapshot?.hasCopiedKeyframe)) {
				send({ type: "paste-keyframe" });
				return;
			}
			if (key === "c" || key === "x") copiedKeyframeRef.current = false;
			send({
				type: "clipboard",
				operation: key === "c" ? "copy" : key === "x" ? "cut" : "paste",
			});
		};
		window.addEventListener("keydown", handleKeyDown);
		return () => window.removeEventListener("keydown", handleKeyDown);
	}, [send, snapshot?.selectedKeyframeId, snapshot?.hasCopiedKeyframe]);

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
			<div
				className="flex shrink-0 items-center gap-1 overflow-x-auto pb-2"
				role="toolbar"
				aria-label="Timeline tools"
			>
				<DropdownMenu>
					<DropdownMenuTrigger asChild>
						<Button variant="ghost" size="sm" className="h-9 shrink-0 gap-2 px-3">
							<Plus className="h-3.5 w-3.5" />
							<span>{t("editor.toolbar.addLayer")}</span>
							<CaretDown className="h-3 w-3" />
						</Button>
					</DropdownMenuTrigger>
					<DropdownMenuContent align="start">
						<DropdownMenuItem
							onClick={() => {
								const nextTrack = snapshot.annotationRegions.reduce(
									(max, region) => Math.max(max, (region.trackIndex ?? 0) + 1),
									0,
								);
								timelineRef.current?.addAnnotation(nextTrack);
							}}
						>
							{t("timeline.annotation.label")}
						</DropdownMenuItem>
						<DropdownMenuItem
							onClick={() => {
								send({ type: "audio-track-added" });
							}}
						>
							Add audio track
						</DropdownMenuItem>
						<DropdownMenuItem onClick={() => void timelineRef.current?.addAudio(Math.max(0, snapshot.audioTrackCount - 1))}>
							Add sound file
						</DropdownMenuItem>
					</DropdownMenuContent>
				</DropdownMenu>
						<Button variant="ghost" size="sm" className="h-9 shrink-0" aria-pressed={snapshot.showSourceAudioTrack} onClick={() => send({ type: "toggle-source-audio-tracks" })}>{snapshot.showSourceAudioTrack ? "Hide source audio" : "Show source audio"}</Button>
				<Button
					variant="ghost"
					size="icon"
					className="h-9 w-9 shrink-0"
					title={t("timeline.zoom.addZoom")}
					aria-label={t("timeline.zoom.addZoom")}
					onClick={() => timelineRef.current?.addZoom()}
				>
					<MagnifyingGlassPlus className="h-4 w-4" />
				</Button>
				<Button
					variant="ghost"
					size="icon"
					className="h-9 w-9 shrink-0"
					title={t("timeline.zoom.suggestZooms")}
					aria-label={t("timeline.zoom.suggestZooms")}
					onClick={() => timelineRef.current?.suggestZooms()}
				>
					<MagicWand className="h-4 w-4" />
				</Button>
				<Button
					variant="ghost"
					size="icon"
					className={`h-9 w-9 shrink-0 ${snapshot.keyframeMode === "auto" ? "bg-primary/20 text-primary" : ""}`}
					title={
						snapshot.keyframeMode === "auto"
							? "Automatic keyframes on — click to turn off; I adds a keyframe manually"
							: "Automatic keyframes off — click to turn on; I adds a keyframe manually"
					}
					aria-label={
						snapshot.keyframeMode === "auto"
							? "Disable automatic keyframes"
							: "Enable automatic keyframes"
					}
					aria-pressed={snapshot.keyframeMode === "auto"}
					onClick={() => send({ type: "toggle-keyframe-mode" })}
				>
					<Diamond
						className="h-4 w-4"
						weight={snapshot.keyframeMode === "auto" ? "fill" : "regular"}
					/>
				</Button>
				<Button
					variant="ghost"
					size="icon"
					className={`h-9 w-9 shrink-0 ${snapshot.showKeyframes ? "bg-primary/20 text-primary" : ""}`}
					title={snapshot.showKeyframes ? "Keyframes visible — click to hide markers" : "Keyframes hidden — click to show markers"}
					aria-label={snapshot.showKeyframes ? "Hide keyframes" : "Show keyframes"}
					aria-pressed={snapshot.showKeyframes}
					onClick={() => send({ type: "toggle-keyframe-visibility" })}
				>
					{snapshot.showKeyframes ? <Eye className="h-4 w-4" /> : <EyeSlash className="h-4 w-4" />}
				</Button>
				<Button
					variant="ghost"
					size="icon"
					className="h-9 w-9 shrink-0"
					title={t("editor.toolbar.splitClip")}
					aria-label={t("editor.toolbar.splitClip")}
					onClick={() => timelineRef.current?.splitClip()}
				>
					<Scissors className="h-4 w-4" />
				</Button>
				<Button
					variant="ghost"
					size="icon"
					className="h-9 w-9 shrink-0"
					title={
						snapshot.isGeneratingCaptions
							? t("settings.captions.generating", "Generating...")
							: t("settings.captions.generateFull", "Generate Captions")
					}
					aria-label={t("settings.captions.generateFull", "Generate Captions")}
					disabled={!snapshot.canGenerateAutoCaptions || snapshot.isGeneratingCaptions}
					onClick={() => send({ type: "generate-captions" })}
				>
					<TextT className="h-4 w-4" />
				</Button>
			</div>
			<div className="flex min-h-0 flex-1 flex-col">
				<TimelineEditor
					ref={timelineRef}
					showAllAnnotationKeyframes={snapshot.showKeyframes}
					selectedKeyframeId={snapshot.selectedKeyframeId}
					onSelectKeyframe={(id) => send({ type: "select-keyframe", id })}
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
					audioTrackCount={snapshot.audioTrackCount}
					onAddAudioTrack={() => send({ type: "audio-track-added" })}
					onDeleteAudioTrack={(trackIndex) => {
						const count = snapshot.audioRegions.filter((region) => (region.trackIndex ?? 0) === trackIndex).length;
						if (count && !window.confirm(`Delete audio track and its ${count} sound clip${count === 1 ? "" : "s"}?`)) return;
						send({ type: "audio-track-delete", trackIndex, confirmed: true });
					}}
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
					onDeleteAnnotationTrack={(trackIndex) => {
						const count = snapshot.annotationRegions.filter((region) => (region.trackIndex ?? 0) === trackIndex).length;
						if (count && !window.confirm(`Delete annotation track and its ${count} annotation${count === 1 ? "" : "s"}?`)) return;
						send({ type: "annotation-track-delete", trackIndex, confirmed: true });
					}}
					onAnnotationPositionKeyframesChange={(id, keyframes) =>
						send({ type: "annotation-position-keyframes-change", id, keyframes })
					}
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
