import type { Span } from "dnd-timeline";
import { type Dispatch, type MutableRefObject, type SetStateAction, useCallback } from "react";
import {
	type AnnotationPositionKeyframe,
	type AnnotationRegion,
	DEFAULT_ANNOTATION_POSITION,
	DEFAULT_ANNOTATION_SIZE,
	DEFAULT_ANNOTATION_STYLE,
	DEFAULT_DRAWING_DATA,
	DEFAULT_FIGURE_DATA,
	DEFAULT_CLICK_HIGHLIGHT_DATA,
	type FigureData,
	type DrawingData,
	type ClickHighlightData,
	type AnnotationStages,
	DEFAULT_ANNOTATION_STAGES,
} from "../types";

interface UseAnnotationRegionCommandsParams {
	onSelectAnnotation: (id: string | null) => void;
	setAnnotationRegions: Dispatch<SetStateAction<AnnotationRegion[]>>;
	selectedAnnotationId: string | null;
	setSelectedAnnotationId: Dispatch<SetStateAction<string | null>>;
	setSelectedZoomId: Dispatch<SetStateAction<string | null>>;
	nextAnnotationIdRef: MutableRefObject<number>;
	nextAnnotationZIndexRef: MutableRefObject<number>;
}

export function useAnnotationRegionCommands({
	onSelectAnnotation,
	setAnnotationRegions,
	selectedAnnotationId,
	setSelectedAnnotationId,
	setSelectedZoomId,
	nextAnnotationIdRef,
	nextAnnotationZIndexRef,
}: UseAnnotationRegionCommandsParams) {
	const handleAnnotationAdded = useCallback(
		(span: Span, trackIndex = 0) => {
			const id = `annotation-${nextAnnotationIdRef.current++}`;
			const newRegion: AnnotationRegion = {
				id,
				startMs: Math.round(span.start),
				endMs: Math.round(span.end),
				type: "text",
				content: "Enter text...",
				position: { ...DEFAULT_ANNOTATION_POSITION },
				size: { ...DEFAULT_ANNOTATION_SIZE },
				style: { ...DEFAULT_ANNOTATION_STYLE },
				zIndex: nextAnnotationZIndexRef.current++,
				trackIndex,
				stages: { ...DEFAULT_ANNOTATION_STAGES },
			};
			setAnnotationRegions((current) => [...current, newRegion]);
			onSelectAnnotation(id);
			setSelectedZoomId(null);
		},
		[
			onSelectAnnotation,
			nextAnnotationIdRef,
			nextAnnotationZIndexRef,
			setAnnotationRegions,
			setSelectedZoomId,
		],
	);

	const handleAnnotationSpanChange = useCallback(
		(id: string, span: Span, trackIndex?: number) => {
			const normalizedTrackIndex =
				typeof trackIndex === "number" && Number.isFinite(trackIndex)
					? Math.max(0, Math.floor(trackIndex))
					: undefined;
			setAnnotationRegions((current) =>
				current.map((region) =>
					region.id === id
						? {
								...region,
								startMs: Math.round(span.start),
								endMs: Math.round(span.end),
								...(normalizedTrackIndex === undefined
									? {}
									: { trackIndex: normalizedTrackIndex }),
							}
						: region,
				),
			);
		},
		[setAnnotationRegions],
	);

	const handleAnnotationDelete = useCallback(
		(id: string) => {
			setAnnotationRegions((current) => current.filter((region) => region.id !== id));
			if (selectedAnnotationId === id) setSelectedAnnotationId(null);
		},
		[selectedAnnotationId, setAnnotationRegions, setSelectedAnnotationId],
	);

	const handleAnnotationContentChange = useCallback(
		(id: string, content: string) => {
			setAnnotationRegions((current) =>
				current.map((region) => {
					if (region.id !== id) return region;
					if (region.type === "text") return { ...region, content, textContent: content };
					if (region.type === "image")
						return { ...region, content, imageContent: content };
					return { ...region, content };
				}),
			);
		},
		[setAnnotationRegions],
	);

	const handleAnnotationTypeChange = useCallback(
		(id: string, type: AnnotationRegion["type"]) => {
			setAnnotationRegions((current) =>
				current.map((region) => {
					if (region.id !== id) return region;
					const updated = { ...region, type };
					if (type === "text") updated.content = region.textContent || "Enter text...";
					else if (type === "image") updated.content = region.imageContent || "";
					else if (type === "figure") {
						updated.content = "";
						if (!region.figureData) updated.figureData = { ...DEFAULT_FIGURE_DATA };
					} else if (type === "blur") {
						updated.content = "";
						if (region.blurIntensity === undefined) updated.blurIntensity = 20;
					} else if (type === "drawing") {
						updated.content = "";
						if (!region.drawingData) {
							updated.drawingData = { ...DEFAULT_DRAWING_DATA, points: [] };
						}
					} else if (type === "click") {
						updated.content = "";
						if (!region.clickData)
							updated.clickData = { ...DEFAULT_CLICK_HIGHLIGHT_DATA };
					} else if (type === "cursor-callout") {
						updated.content = "";
						if (!region.calloutEvent) updated.calloutEvent = { action: "enable" };
					}
					return updated;
				}),
			);
		},
		[setAnnotationRegions],
	);

	const updateRegion = useCallback(
		(id: string, patch: Partial<AnnotationRegion>) => {
			setAnnotationRegions((current) =>
				current.map((region) => (region.id === id ? { ...region, ...patch } : region)),
			);
		},
		[setAnnotationRegions],
	);

	const handleAnnotationStyleChange = useCallback(
		(id: string, style: Partial<AnnotationRegion["style"]>) => {
			setAnnotationRegions((current) =>
				current.map((region) =>
					region.id === id ? { ...region, style: { ...region.style, ...style } } : region,
				),
			);
		},
		[setAnnotationRegions],
	);
	const handleAnnotationFigureDataChange = useCallback(
		(id: string, figureData: FigureData) => updateRegion(id, { figureData }),
		[updateRegion],
	);

	const handleAnnotationDuplicate = useCallback(
		(region: AnnotationRegion) => {
			const duration = Math.max(1, region.endMs - region.startMs);
			const id = `annotation-${nextAnnotationIdRef.current++}`;
			setAnnotationRegions((current) => {
				const lastEnd = current
					.filter((candidate) => (candidate.trackIndex ?? 0) === (region.trackIndex ?? 0))
					.reduce((latest, candidate) => Math.max(latest, candidate.endMs), region.endMs);
				const startMs = lastEnd + 100;
				return [
					...current,
					{
						...structuredClone(region),
						id,
						startMs,
						endMs: startMs + duration,
						zIndex: nextAnnotationZIndexRef.current++,
					},
				];
			});
			onSelectAnnotation(id);
		},
		[nextAnnotationIdRef, nextAnnotationZIndexRef, onSelectAnnotation, setAnnotationRegions],
	);
	const handleAnnotationDrawingDataChange = useCallback(
		(id: string, drawingData: DrawingData) => updateRegion(id, { drawingData }),
		[updateRegion],
	);
	const handleAnnotationStagesChange = useCallback(
		(id: string, stages: Partial<AnnotationStages>) => {
			setAnnotationRegions((current) =>
				current.map((region) => {
					if (region.id !== id) return region;
					const duration = Math.max(1, region.endMs - region.startMs);
					const next = { ...DEFAULT_ANNOTATION_STAGES, ...region.stages, ...stages };
					next.inDurationMs = Math.max(
						0,
						Math.min(duration, Math.round(next.inDurationMs)),
					);
					next.outDurationMs = Math.max(
						0,
						Math.min(duration - next.inDurationMs, Math.round(next.outDurationMs)),
					);
					return { ...region, stages: next };
				}),
			);
		},
		[setAnnotationRegions],
	);
	const handleAnnotationBlurIntensityChange = useCallback(
		(id: string, blurIntensity: number) => updateRegion(id, { blurIntensity }),
		[updateRegion],
	);
	const handleAnnotationBlurColorChange = useCallback(
		(id: string, blurColor: string) => updateRegion(id, { blurColor }),
		[updateRegion],
	);
	const handleAnnotationPositionChange = useCallback(
		(id: string, position: { x: number; y: number }) => updateRegion(id, { position }),
		[updateRegion],
	);
	const handleAnnotationClickDataChange = useCallback(
		(id: string, clickData: ClickHighlightData) => updateRegion(id, { clickData }),
		[updateRegion],
	);
	const handleAnnotationCalloutEventChange = useCallback(
		(id: string, calloutEvent: NonNullable<AnnotationRegion["calloutEvent"]>) =>
			updateRegion(id, { calloutEvent }),
		[updateRegion],
	);
	const handleAnnotationPositionKeyframesChange = useCallback(
		(id: string, positionKeyframes: AnnotationPositionKeyframe[]) =>
			updateRegion(id, { positionKeyframes }),
		[updateRegion],
	);
	const handleAnnotationSizeChange = useCallback(
		(id: string, size: { width: number; height: number }) => updateRegion(id, { size }),
		[updateRegion],
	);

	return {
		handleAnnotationAdded,
		handleAnnotationSpanChange,
		handleAnnotationDelete,
		handleAnnotationDuplicate,
		handleAnnotationContentChange,
		handleAnnotationTypeChange,
		handleAnnotationStyleChange,
		handleAnnotationFigureDataChange,
		handleAnnotationDrawingDataChange,
		handleAnnotationClickDataChange,
		handleAnnotationCalloutEventChange,
		handleAnnotationStagesChange,
		handleAnnotationBlurIntensityChange,
		handleAnnotationBlurColorChange,
		handleAnnotationPositionChange,
		handleAnnotationPositionKeyframesChange,
		handleAnnotationSizeChange,
	};
}
