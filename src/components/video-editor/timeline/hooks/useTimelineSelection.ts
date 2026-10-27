import { useCallback, useMemo, useRef, useState } from "react";
import { getAnnotationKeyframeProperties, resolveAnnotationAtTime, resolveAnnotationTransformAtTime } from "../../annotationKeyframes";
import type { AnnotationPositionKeyframe, AnnotationRegion } from "../../types";
import type { TimelineRegion } from "../core/timelineTypes";

interface UseTimelineSelectionParams {
	totalMs: number;
	currentTimeMs: number;
	zoomRegions: TimelineRegion[];
	clipRegions: TimelineRegion[];
	annotationRegions: AnnotationRegion[];
	audioRegions: TimelineRegion[];
	selectedZoomId: string | null;
	selectedClipId?: string | null;
	selectedAnnotationId?: string | null;
	selectedAudioId?: string | null;
	selectedCaptionId?: string | null;
	onZoomDelete: (id: string) => void;
	onClipDelete?: (id: string) => void;
	onAnnotationDelete?: (id: string) => void;
	onAnnotationPositionKeyframesChange?: (
		id: string,
		keyframes: AnnotationPositionKeyframe[],
	) => void;
	selectedKeyframeId?: string | null;
	onSelectKeyframe?: (id: string | null) => void;
	keyframeClipboardRef?: { current: { annotationId: string; frame: AnnotationPositionKeyframe } | null };
	onAudioDelete?: (id: string) => void;
	onCaptionDelete?: (id: string) => void;
	onSelectZoom: (id: string | null) => void;
	onSelectClip?: (id: string | null) => void;
	onSelectAnnotation?: (id: string | null) => void;
	onSelectAudio?: (id: string | null) => void;
	onSelectCaption?: (id: string | null) => void;
}

export function useTimelineSelection({
	totalMs,
	currentTimeMs,
	zoomRegions,
	annotationRegions,
	selectedZoomId,
	selectedClipId,
	selectedAnnotationId,
	selectedAudioId,
	selectedCaptionId,
	onZoomDelete,
	onClipDelete,
	onAnnotationDelete,
	onAnnotationPositionKeyframesChange,
	selectedKeyframeId: sharedSelectedKeyframeId,
	onSelectKeyframe,
	keyframeClipboardRef,
	onAudioDelete,
	onCaptionDelete,
	onSelectZoom,
	onSelectClip,
	onSelectAnnotation,
	onSelectAudio,
	onSelectCaption,
}: UseTimelineSelectionParams) {
	const selectedAnnotation = annotationRegions.find(
		(region) => region.id === selectedAnnotationId,
	);
	const keyframes = useMemo(
		() =>
			(selectedAnnotation?.positionKeyframes ?? []).map((frame) => ({
				id: frame.id,
				time: frame.timeMs,
			})),
		[selectedAnnotation?.positionKeyframes],
	);
	const [localSelectedKeyframeId, setLocalSelectedKeyframeId] = useState<string | null>(null);
	const selectedKeyframeId = sharedSelectedKeyframeId === undefined ? localSelectedKeyframeId : sharedSelectedKeyframeId;
	const setSelectedKeyframeId = onSelectKeyframe ?? setLocalSelectedKeyframeId;
	const localKeyframeClipboardRef = useRef<{ annotationId: string; frame: AnnotationPositionKeyframe } | null>(null);
	const copiedKeyframeRef = keyframeClipboardRef ?? localKeyframeClipboardRef;
	const [selectAllBlocksActive, setSelectAllBlocksActive] = useState(false);
	const hasAnyZoomBlocks = useMemo(() => zoomRegions.length > 0, [zoomRegions.length]);

	const addKeyframe = useCallback(() => {
		if (totalMs === 0 || !selectedAnnotation || !onAnnotationPositionKeyframesChange) return;
		const time = Math.round(Math.max(0, Math.min(currentTimeMs, totalMs)));
		const existing = keyframes.find((kf) => Math.abs(kf.time - time) < 1);
		if (existing) {
			setSelectedKeyframeId(existing.id);
			return;
		}
		const id = globalThis.crypto.randomUUID();
		onAnnotationPositionKeyframesChange(selectedAnnotation.id, [
			...(selectedAnnotation.positionKeyframes ?? []),
			{
				id,
				timeMs: time,
				...resolveAnnotationTransformAtTime(selectedAnnotation, time),
				properties: structuredClone(getAnnotationKeyframeProperties(resolveAnnotationAtTime(selectedAnnotation, time))),
			},
		].sort((left, right) => left.timeMs - right.timeMs));
		setSelectedKeyframeId(id);
	}, [currentTimeMs, totalMs, keyframes, onAnnotationPositionKeyframesChange, selectedAnnotation, setSelectedKeyframeId]);

	const deleteSelectedKeyframe = useCallback(() => {
		if (!selectedKeyframeId) return;
		if (selectedAnnotation && onAnnotationPositionKeyframesChange) {
			onAnnotationPositionKeyframesChange(
				selectedAnnotation.id,
				(selectedAnnotation.positionKeyframes ?? []).filter(
					(frame) => frame.id !== selectedKeyframeId,
				),
			);
		}
		setSelectedKeyframeId(null);
	}, [onAnnotationPositionKeyframesChange, selectedAnnotation, selectedKeyframeId, setSelectedKeyframeId]);

	const copySelectedKeyframe = useCallback(() => {
		const source = annotationRegions.find((region) => region.positionKeyframes?.some((item) => item.id === selectedKeyframeId));
		const frame = source?.positionKeyframes?.find((item) => item.id === selectedKeyframeId);
		if (source && frame) copiedKeyframeRef.current = { annotationId: source.id, frame: structuredClone(frame) };
	}, [annotationRegions, copiedKeyframeRef, selectedKeyframeId]);
	const hasCopiedKeyframe = useCallback(() => copiedKeyframeRef.current !== null, [copiedKeyframeRef]);

	const pasteKeyframe = useCallback(() => {
		const copied = copiedKeyframeRef.current;
		if (!copied || !onAnnotationPositionKeyframesChange) return;
		const target = selectedAnnotation ?? annotationRegions.find((region) => region.id === copied.annotationId);
		if (!target) return;
		const timeMs = Math.max(0, Math.min(totalMs, Math.round(currentTimeMs)));
		const next = (target.positionKeyframes ?? []).filter((frame) => Math.abs(frame.timeMs - timeMs) >= 1);
		const pasted = { ...structuredClone(copied.frame), id: globalThis.crypto.randomUUID(), timeMs };
		onAnnotationPositionKeyframesChange(target.id, [...next, pasted].sort((a, b) => a.timeMs - b.timeMs));
		onSelectAnnotation?.(target.id);
		setSelectedKeyframeId(pasted.id);
	}, [annotationRegions, copiedKeyframeRef, currentTimeMs, onAnnotationPositionKeyframesChange, onSelectAnnotation, selectedAnnotation, totalMs, setSelectedKeyframeId]);

	const handleKeyframeMove = useCallback(
		(id: string, newTime: number) => {
			if (!selectedAnnotation || !onAnnotationPositionKeyframesChange) return;
			onAnnotationPositionKeyframesChange(
				selectedAnnotation.id,
				(selectedAnnotation.positionKeyframes ?? [])
					.map((frame) =>
						frame.id === id
							? { ...frame, timeMs: Math.max(0, Math.min(newTime, totalMs)) }
							: frame,
					)
					.sort((left, right) => left.timeMs - right.timeMs),
			);
		},
		[onAnnotationPositionKeyframesChange, selectedAnnotation, totalMs],
	);

	const deleteSelectedZoom = useCallback(() => {
		if (selectAllBlocksActive) {
			zoomRegions.map((region) => region.id).forEach((id) => onZoomDelete(id));
		} else if (selectedZoomId) {
			onZoomDelete(selectedZoomId);
		} else {
			return;
		}

		onSelectZoom(null);
		onSelectClip?.(null);
		onSelectAnnotation?.(null);
		onSelectAudio?.(null);
		onSelectCaption?.(null);
		setSelectAllBlocksActive(false);
	}, [
		selectAllBlocksActive,
		zoomRegions,
		onZoomDelete,
		selectedZoomId,
		onSelectZoom,
		onSelectClip,
		onSelectAnnotation,
		onSelectAudio,
		onSelectCaption,
	]);

	const deleteSelectedClip = useCallback(() => {
		if (!selectedClipId || !onClipDelete) return;
		onClipDelete(selectedClipId);
		onSelectClip?.(null);
	}, [selectedClipId, onClipDelete, onSelectClip]);

	const deleteSelectedAnnotation = useCallback(() => {
		if (!selectedAnnotationId || !onAnnotationDelete) return;
		onAnnotationDelete(selectedAnnotationId);
		onSelectAnnotation?.(null);
	}, [selectedAnnotationId, onAnnotationDelete, onSelectAnnotation]);

	const deleteSelectedAudio = useCallback(() => {
		if (!selectedAudioId || !onAudioDelete) return;
		onAudioDelete(selectedAudioId);
		onSelectAudio?.(null);
	}, [selectedAudioId, onAudioDelete, onSelectAudio]);

	const deleteSelectedCaption = useCallback(() => {
		if (!selectedCaptionId || !onCaptionDelete) return;
		onCaptionDelete(selectedCaptionId);
		onSelectCaption?.(null);
	}, [selectedCaptionId, onCaptionDelete, onSelectCaption]);

	const clearSelectedBlocks = useCallback(() => {
		onSelectZoom(null);
		onSelectClip?.(null);
		onSelectAnnotation?.(null);
		onSelectAudio?.(null);
		onSelectCaption?.(null);
		setSelectAllBlocksActive(false);
	}, [onSelectZoom, onSelectClip, onSelectAnnotation, onSelectAudio, onSelectCaption]);

	const activateSelectAllZooms = useCallback(() => {
		onSelectZoom(null);
		onSelectClip?.(null);
		onSelectAnnotation?.(null);
		onSelectAudio?.(null);
		onSelectCaption?.(null);
		setSelectedKeyframeId(null);
		setSelectAllBlocksActive(true);
	}, [onSelectZoom, onSelectClip, onSelectAnnotation, onSelectAudio, onSelectCaption, setSelectedKeyframeId]);

	const handleSelectZoom = useCallback(
		(id: string | null) => {
			setSelectAllBlocksActive(false);
			setSelectedKeyframeId(null);
			onSelectZoom(id);
		},
		[onSelectZoom, setSelectedKeyframeId],
	);

	const handleSelectClip = useCallback(
		(id: string | null) => {
			setSelectAllBlocksActive(false);
			setSelectedKeyframeId(null);
			onSelectClip?.(id);
		},
		[onSelectClip, setSelectedKeyframeId],
	);

	const handleSelectAnnotation = useCallback(
		(id: string | null) => {
			setSelectAllBlocksActive(false);
			setSelectedKeyframeId(null);
			onSelectAnnotation?.(id);
		},
		[onSelectAnnotation, setSelectedKeyframeId],
	);

	const handleSelectAudio = useCallback(
		(id: string | null) => {
			setSelectAllBlocksActive(false);
			setSelectedKeyframeId(null);
			onSelectAudio?.(id);
		},
		[onSelectAudio, setSelectedKeyframeId],
	);

	const handleSelectCaption = useCallback(
		(id: string | null) => {
			setSelectAllBlocksActive(false);
			setSelectedKeyframeId(null);
			onSelectCaption?.(id);
		},
		[onSelectCaption, setSelectedKeyframeId],
	);

	const cycleAnnotationsAtCurrentTime = useCallback(
		(backward = false) => {
			const overlapping = annotationRegions
				.filter((a) => currentTimeMs >= a.startMs && currentTimeMs <= a.endMs)
				.sort((a, b) => a.zIndex - b.zIndex);
			if (overlapping.length === 0) {
				return false;
			}

			if (!selectedAnnotationId || !overlapping.some((a) => a.id === selectedAnnotationId)) {
				onSelectAnnotation?.(overlapping[0].id);
				return true;
			}

			const currentIndex = overlapping.findIndex((a) => a.id === selectedAnnotationId);
			const nextIndex = backward
				? (currentIndex - 1 + overlapping.length) % overlapping.length
				: (currentIndex + 1) % overlapping.length;
			onSelectAnnotation?.(overlapping[nextIndex].id);
			return true;
		},
		[annotationRegions, currentTimeMs, selectedAnnotationId, onSelectAnnotation],
	);

	return {
		keyframes,
		selectedKeyframeId,
		setSelectedKeyframeId,
		selectAllBlocksActive,
		setSelectAllBlocksActive,
		hasAnyZoomBlocks,
		activateSelectAllZooms,
		addKeyframe,
		deleteSelectedKeyframe,
		copySelectedKeyframe,
		hasCopiedKeyframe,
		pasteKeyframe,
		handleKeyframeMove,
		deleteSelectedZoom,
		deleteSelectedClip,
		deleteSelectedAnnotation,
		deleteSelectedAudio,
		deleteSelectedCaption,
		clearSelectedBlocks,
		handleSelectZoom,
		handleSelectClip,
		handleSelectAnnotation,
		handleSelectAudio,
		handleSelectCaption,
		cycleAnnotationsAtCurrentTime,
	};
}
