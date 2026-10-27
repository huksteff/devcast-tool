import { type RefObject, useEffect, useRef } from "react";
import type { useShortcuts } from "@/contexts/ShortcutsContext";
import { matchesShortcut } from "@/lib/shortcuts";
import type { useTimelineState } from "../state/useTimelineState";
import type { VideoPlaybackRef } from "../VideoPlayback";
import type { useAnnotationRegionCommands } from "./useAnnotationRegionCommands";
import type { useZoomRegionCommands } from "./useZoomRegionCommands";

type Input = {
	timeline: ReturnType<typeof useTimelineState>;
	videoPlaybackRef: RefObject<VideoPlaybackRef | null>;
	shortcuts: ReturnType<typeof useShortcuts>["shortcuts"];
	isMac: boolean;
	handleUndo: () => void;
	handleRedo: () => void;
	startPlayback: () => void;
	stepFrame: (direction: -1 | 1) => void;
	annotationCommands: ReturnType<typeof useAnnotationRegionCommands>;
	zoomCommands: ReturnType<typeof useZoomRegionCommands>;
};

export function useEditorGlobalInteractions({
	timeline,
	videoPlaybackRef,
	shortcuts,
	isMac,
	handleUndo,
	handleRedo,
	startPlayback,
	stepFrame,
	annotationCommands,
	zoomCommands,
}: Input) {
	const heldPlaybackKey = useRef<string | null>(null);
	const timelineClipboard = useRef<
		| { kind: "annotation"; region: (typeof timeline.annotationRegions)[number] }
		| { kind: "zoom"; region: (typeof timeline.zoomRegions)[number] }
		| null
	>(null);

	useEffect(() => {
		const consumePlaybackKey = (event: KeyboardEvent) => {
			event.preventDefault();
			// React Aria buttons also handle Space. Own both halves of this shortcut
			// so their press handler cannot toggle playback a second time.
			event.stopImmediatePropagation();
		};
		const keyIdentity = (event: KeyboardEvent) => event.code || event.key.toLowerCase();
		const handleKeyUp = (event: KeyboardEvent) => {
			if (heldPlaybackKey.current !== keyIdentity(event)) return;
			heldPlaybackKey.current = null;
			consumePlaybackKey(event);
		};
		const releasePlaybackKey = () => {
			heldPlaybackKey.current = null;
		};
		const handleKeyDown = (event: KeyboardEvent) => {
			if (heldPlaybackKey.current === keyIdentity(event)) {
				consumePlaybackKey(event);
				return;
			}
			if (event.isComposing || event.defaultPrevented) return;
			const target = event.target as HTMLElement | null;
			if (target?.closest?.("[data-recording-library]")) return;
			const editable =
				(target instanceof HTMLInputElement &&
					!["checkbox", "radio", "range", "button", "submit", "reset"].includes(
						target.type,
					)) ||
				target instanceof HTMLTextAreaElement ||
				target instanceof HTMLSelectElement ||
				target?.isContentEditable;
			const primaryModifier = isMac ? event.metaKey : event.ctrlKey;
			const key = event.key.toLowerCase();
			// `event.key` follows the keyboard layout (Ctrl+Я on Russian is KeyZ).
			const physicalKey = event.code.toLowerCase();
			if (primaryModifier && (key === "c" || physicalKey === "keyc") && window.getSelection?.()?.toString()) return;
			if (
				!editable &&
				!(target instanceof HTMLInputElement && target.type === "range") &&
				!event.altKey &&
				!event.ctrlKey &&
				!event.metaKey &&
				(event.key === "ArrowLeft" || event.key === "ArrowRight")
			) {
				event.preventDefault();
				event.stopImmediatePropagation();
				stepFrame(event.key === "ArrowLeft" ? -1 : 1);
				return;
			}
			if (
				primaryModifier &&
				!event.altKey &&
				(key === "c" || key === "x" || key === "v") &&
				!editable
			) {
				// Keyframe copy/paste is handled by the timeline shortcut listener.
				if ((key === "c" && timeline.selectedKeyframeId) || (key === "v" && timeline.keyframeClipboardRef.current)) return;
				if (key === "v") {
					if (!timelineClipboard.current) return;
					event.preventDefault();
					if (timelineClipboard.current.kind === "annotation")
						annotationCommands.handleAnnotationDuplicate(
							timelineClipboard.current.region,
						);
					else zoomCommands.handleZoomDuplicate(timelineClipboard.current.region);
					return;
				}
				const annotation = timeline.selectedAnnotationId
					? timeline.annotationRegions.find(
							(region) => region.id === timeline.selectedAnnotationId,
						)
					: undefined;
				const zoom = timeline.selectedZoomId
					? timeline.zoomRegions.find((region) => region.id === timeline.selectedZoomId)
					: undefined;
				if (!annotation && !zoom) return;
				event.preventDefault();
				timeline.keyframeClipboardRef.current = null;
				if (annotation) {
					timelineClipboard.current = {
						kind: "annotation",
						region: structuredClone(annotation),
					};
					if (key === "x") annotationCommands.handleAnnotationDelete(annotation.id);
				} else if (zoom) {
					timelineClipboard.current = { kind: "zoom", region: structuredClone(zoom) };
					if (key === "x") zoomCommands.handleZoomDelete(zoom.id);
				}
				return;
			}

			if (primaryModifier && !event.altKey && (key === "z" || physicalKey === "keyz")) {
				if (!editable) {
					event.preventDefault();
					if (event.shiftKey) handleRedo();
					else handleUndo();
				}
				return;
			}
			if (
				!isMac &&
				event.ctrlKey &&
				!event.metaKey &&
				!event.altKey &&
				(key === "y" || physicalKey === "keyy")
			) {
				if (!editable) {
					event.preventDefault();
					handleRedo();
				}
				return;
			}
			// Space always controls preview playback, even when a just-clicked switch or
			// button still owns focus. This prevents it from toggling that control again.
			const isSpace = event.code === "Space" && shortcuts.playPause.key === " ";
			if ((!matchesShortcut(event, shortcuts.playPause, isMac) && !isSpace) || editable)
				return;
			consumePlaybackKey(event);
			if (event.repeat) return;
			heldPlaybackKey.current = keyIdentity(event);
			const playback = videoPlaybackRef.current;
			if (!playback?.video) return;
			if (!playback.isPlaying) startPlayback();
			else playback.pause();
		};
		window.addEventListener("keydown", handleKeyDown, {
			capture: true,
		});
		window.addEventListener("keyup", handleKeyUp, { capture: true });
		window.addEventListener("blur", releasePlaybackKey);
		return () => {
			window.removeEventListener("keydown", handleKeyDown, {
				capture: true,
			});
			window.removeEventListener("keyup", handleKeyUp, {
				capture: true,
			});
			window.removeEventListener("blur", releasePlaybackKey);
		};
	}, [
		annotationCommands,
		shortcuts,
		isMac,
		handleUndo,
		handleRedo,
		startPlayback,
		stepFrame,
		timeline,
		videoPlaybackRef,
		zoomCommands,
	]);

	useEffect(() => {
		if (
			timeline.selectedZoomId &&
			!timeline.zoomRegions.some(({ id }) => id === timeline.selectedZoomId)
		) {
			timeline.setSelectedZoomId(null);
		}
	}, [timeline.selectedZoomId, timeline.zoomRegions, timeline.setSelectedZoomId]);
	useEffect(() => {
		if (
			timeline.selectedAnnotationId &&
			!timeline.annotationRegions.some(({ id }) => id === timeline.selectedAnnotationId)
		) {
			timeline.setSelectedAnnotationId(null);
		}
	}, [
		timeline.selectedAnnotationId,
		timeline.annotationRegions,
		timeline.setSelectedAnnotationId,
	]);
	useEffect(() => {
		if (
			timeline.selectedAudioId &&
			!timeline.audioRegions.some(({ id }) => id === timeline.selectedAudioId)
		) {
			timeline.setSelectedAudioId(null);
		}
	}, [timeline.selectedAudioId, timeline.audioRegions, timeline.setSelectedAudioId]);
	useEffect(() => {
		if (
			timeline.selectedCaptionId &&
			!timeline.autoCaptions.some(({ id }) => id === timeline.selectedCaptionId)
		) {
			timeline.setSelectedCaptionId(null);
		}
	}, [timeline.selectedCaptionId, timeline.autoCaptions, timeline.setSelectedCaptionId]);
}
