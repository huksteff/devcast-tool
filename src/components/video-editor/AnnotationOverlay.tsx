import { useRef } from "react";
import { Rnd } from "react-rnd";
import { cn } from "@/lib/utils";
import { getArrowComponent } from "./ArrowSvgs";
import {
	type AnnotationRegion,
	type DrawingData,
	BASE_PREVIEW_WIDTH,
	BLUR_ANNOTATION_STRENGTH,
	getAnnotationStageState,
	getVisibleDrawingStrokes,
} from "./types";

type Rect = {
	x: number;
	y: number;
	width: number;
	height: number;
};

type SceneTransform = {
	scale: number;
	x: number;
	y: number;
};

interface AnnotationOverlayProps {
	annotation: AnnotationRegion;
	isSelected: boolean;
	containerWidth: number;
	containerHeight: number;
	recordingRect: Rect;
	sceneTransform: SceneTransform;
	interactionScale?: number;
	onPositionChange: (id: string, position: { x: number; y: number }) => void;
	onSizeChange: (id: string, size: { width: number; height: number }) => void;
	onDrawingChange?: (id: string, drawing: DrawingData) => void;
	onClick: (id: string) => void;
	zIndex: number;
	isSelectedBoost: boolean; // Boost z-index when selected for easy editing
	currentTimeMs?: number;
}

function positivePercent(value: number) {
	if (!Number.isFinite(value)) {
		return 0;
	}

	return Math.max(1, value);
}

/** Render an annotation in preview space with editor drag and resize controls. */
export function AnnotationOverlay({
	annotation,
	isSelected,
	containerWidth,
	containerHeight,
	recordingRect,
	sceneTransform,
	interactionScale = 1,
	onPositionChange,
	onSizeChange,
	onDrawingChange,
	onClick,
	zIndex,
	isSelectedBoost,
	currentTimeMs = annotation.startMs,
}: AnnotationOverlayProps) {
	const safeRecordingRect =
		recordingRect.width > 0 && recordingRect.height > 0
			? recordingRect
			: { x: 0, y: 0, width: containerWidth, height: containerHeight };
	const sceneX = safeRecordingRect.x + (annotation.position.x / 100) * safeRecordingRect.width;
	const sceneY = safeRecordingRect.y + (annotation.position.y / 100) * safeRecordingRect.height;
	const sceneWidth = (annotation.size.width / 100) * safeRecordingRect.width;
	const sceneHeight = (annotation.size.height / 100) * safeRecordingRect.height;
	const x = sceneX * sceneTransform.scale + sceneTransform.x;
	const y = sceneY * sceneTransform.scale + sceneTransform.y;
	const width = sceneWidth * sceneTransform.scale;
	const height = sceneHeight * sceneTransform.scale;
	const sizeScale = safeRecordingRect.width / BASE_PREVIEW_WIDTH;
	const blurScaleFactor = sizeScale * sceneTransform.scale;
	const stage = getAnnotationStageState(annotation, currentTimeMs);

	const isDraggingRef = useRef(false);
	const drawingRef = useRef<DrawingData | null>(null);
	const activePointerRef = useRef<number | null>(null);

	const screenRectToRecordingPercent = (rect: Rect) => {
		const nextSceneX = (rect.x - sceneTransform.x) / sceneTransform.scale;
		const nextSceneY = (rect.y - sceneTransform.y) / sceneTransform.scale;
		const nextSceneWidth = rect.width / sceneTransform.scale;
		const nextSceneHeight = rect.height / sceneTransform.scale;

		return {
			position: {
				x:
					((nextSceneX - safeRecordingRect.x) / Math.max(1, safeRecordingRect.width)) *
					100,
				y:
					((nextSceneY - safeRecordingRect.y) / Math.max(1, safeRecordingRect.height)) *
					100,
			},
			size: {
				width: positivePercent(
					(nextSceneWidth / Math.max(1, safeRecordingRect.width)) * 100,
				),
				height: positivePercent(
					(nextSceneHeight / Math.max(1, safeRecordingRect.height)) * 100,
				),
			},
		};
	};

	const renderArrow = () => {
		const direction = annotation.figureData?.arrowDirection || "right";
		const color = annotation.figureData?.color || "#2563EB";
		const strokeWidth = annotation.figureData?.strokeWidth || 4;

		const ArrowComponent = getArrowComponent(direction);
		return <ArrowComponent color={color} strokeWidth={strokeWidth} />;
	};

	const renderContent = () => {
		switch (annotation.type) {
			case "text":
				return (
					<div
						className="w-full h-full flex items-center overflow-hidden"
						style={{
							justifyContent:
								annotation.style.textAlign === "left"
									? "flex-start"
									: annotation.style.textAlign === "right"
										? "flex-end"
										: "center",
							alignItems: "center",
							padding: `${8 * sizeScale}px`,
						}}
					>
						<span
							style={{
								color: annotation.style.color,
								backgroundColor: annotation.style.backgroundColor,
								fontSize: `${annotation.style.fontSize * sizeScale}px`,
								fontFamily: annotation.style.fontFamily,
								fontWeight: annotation.style.fontWeight,
								fontStyle: annotation.style.fontStyle,
								textDecoration: annotation.style.textDecoration,
								textAlign: annotation.style.textAlign,
								wordBreak: "break-word",
								whiteSpace: "pre-wrap",
								boxDecorationBreak: "clone",
								WebkitBoxDecorationBreak: "clone",
								padding: "0.1em 0.2em",
								borderRadius: `${4 * sizeScale}px`,
								lineHeight: "1.4",
							}}
						>
							{annotation.content.slice(0, Math.ceil(annotation.content.length * stage.textProgress))}
						</span>
					</div>
				);

			case "image":
				if (annotation.content && annotation.content.startsWith("data:image")) {
					return (
						<img
							src={annotation.content}
							alt="Annotation"
							className="w-full h-full object-contain"
							draggable={false}
						/>
					);
				}
				return (
					<div className="w-full h-full flex items-center justify-center text-muted-foreground text-sm">
						No image
					</div>
				);

			case "figure":
				if (!annotation.figureData) {
					return (
						<div className="w-full h-full flex items-center justify-center text-muted-foreground text-sm">
							No arrow data
						</div>
					);
				}
				if ((annotation.figureData.kind ?? "arrow") !== "arrow") {
					const kind = annotation.figureData.kind ?? "rectangle";
					const common = {
						stroke: annotation.figureData.color,
						strokeWidth: annotation.figureData.strokeWidth,
						fill: "none",
					};
					return (
						<svg className="h-full w-full" viewBox="0 0 100 100" preserveAspectRatio="none">
							{kind === "ellipse" && <ellipse cx="50" cy="50" rx="47" ry="47" {...common} />}
							{kind === "line" && <line x1="3" y1="97" x2="97" y2="3" {...common} strokeLinecap="round" />}
							{kind === "highlight" && <rect width="100" height="100" fill={annotation.figureData.color} opacity="0.28" />}
							{kind === "rectangle" && <rect x="3" y="3" width="94" height="94" {...common} />}
						</svg>
					);
				}

				return (
					<div
						className="w-full h-full flex items-center justify-center"
						style={{ padding: `${8 * sizeScale}px` }}
					>
						{renderArrow()}
					</div>
				);

			case "blur": {
				const currentBlurStrength = annotation.blurIntensity ?? BLUR_ANNOTATION_STRENGTH;
				const blurPx = currentBlurStrength * blurScaleFactor;
				const blurStyle = `blur(${blurPx}px)`;

				return (
					<div
						className="h-full w-full bg-slate-400/10"
						style={{
							backdropFilter: blurStyle,
							WebkitBackdropFilter: blurStyle,
							backgroundColor: annotation.blurColor || "transparent",
							borderRadius: `${(annotation.style.borderRadius ?? 0) * blurScaleFactor}px`,
						}}
					/>
				);
			}

			case "drawing": {
				const drawing = annotation.drawingData;
				const strokes = drawing?.strokes?.length ? drawing.strokes : drawing?.points?.length ? [drawing.points] : [];
				// Keep input responsive while editing the current path. At playback the
				// stage transition determines exactly how much of the drawing is shown.
				const visibleStrokes = isSelected && activePointerRef.current !== null
					? strokes
					: getVisibleDrawingStrokes(drawing, stage.drawingProgress);
				const getPoint = (event: React.PointerEvent<SVGSVGElement>) => {
					const rect = event.currentTarget.getBoundingClientRect();
					return {
						x: Math.max(0, Math.min(100, ((event.clientX - rect.left) / Math.max(1, rect.width)) * 100)),
						y: Math.max(0, Math.min(100, ((event.clientY - rect.top) / Math.max(1, rect.height)) * 100)),
					};
				};
				const updateStroke = (event: React.PointerEvent<SVGSVGElement>, start: boolean) => {
					if (!isSelected || !onDrawingChange) return;
					event.stopPropagation();
					const point = getPoint(event);
					const previousStrokes = drawingRef.current?.strokes ?? strokes;
					const nextStrokes = start
						? [...previousStrokes, [point]]
						: previousStrokes.length
							? [
									...previousStrokes.slice(0, -1),
									[...previousStrokes[previousStrokes.length - 1], point],
								]
							: [[point]];
					const next: DrawingData = {
						color: drawing?.color ?? "#2563EB",
						strokeWidth: drawing?.strokeWidth ?? 4,
						// Keep points for backwards-compatible consumers; new rendering uses strokes.
						points: nextStrokes.flat(),
						strokes: nextStrokes,
					};
					drawingRef.current = next;
					onDrawingChange(annotation.id, next);
				};
				return (
					<svg
						className={isSelected ? "h-full w-full touch-none cursor-crosshair" : "h-full w-full"}
						viewBox="0 0 100 100"
						preserveAspectRatio="none"
						onPointerDown={(event) => {
							if (!isSelected || !onDrawingChange) return;
							event.stopPropagation();
							if (event.button === 2) {
								// The secondary button is an intentional clear gesture, not a browser menu.
								onDrawingChange(annotation.id, {
									color: drawing?.color ?? "#2563EB",
									strokeWidth: drawing?.strokeWidth ?? 4,
									points: [],
									strokes: [],
								});
								drawingRef.current = null;
								return;
							}
							if (event.button !== 0) return;
							activePointerRef.current = event.pointerId;
							event.currentTarget.setPointerCapture(event.pointerId);
							updateStroke(event, true);
						}}
						onPointerMove={(event) => {
							if (activePointerRef.current === event.pointerId && event.currentTarget.hasPointerCapture(event.pointerId)) updateStroke(event, false);
						}}
						onPointerUp={(event) => {
							if (activePointerRef.current !== event.pointerId) return;
							drawingRef.current = null;
							activePointerRef.current = null;
							event.currentTarget.releasePointerCapture(event.pointerId);
						}}
						onPointerCancel={() => { drawingRef.current = null; activePointerRef.current = null; }}
						onContextMenu={(event) => {
							event.preventDefault();
						}}
					>
						{visibleStrokes.map((stroke, index) => {
							const points = stroke.map((point) => `${point.x},${point.y}`).join(" ");
							return stroke.length === 1 ? (
								<circle key={index} cx={stroke[0].x} cy={stroke[0].y} r={(drawing?.strokeWidth ?? 4) / 2} fill={drawing?.color ?? "#2563EB"} vectorEffect="non-scaling-stroke" />
							) : (
								<polyline key={index} points={points} fill="none" stroke={drawing?.color ?? "#2563EB"} strokeWidth={drawing?.strokeWidth ?? 4} vectorEffect="non-scaling-stroke" strokeLinecap="round" strokeLinejoin="round" />
							);
						})}
					</svg>
				);
			}

			default:
				return null;
		}
	};

	return (
		<Rnd
			data-annotation-id={annotation.id}
			position={{ x, y }}
			size={{ width, height }}
			scale={interactionScale}
			onDragStart={() => {
				isDraggingRef.current = true;
			}}
			onDragStop={(_e, d) => {
				const next = screenRectToRecordingPercent({ x: d.x, y: d.y, width, height });
				onPositionChange(annotation.id, next.position);

				// Reset dragging flag after a short delay to prevent click event
				setTimeout(() => {
					isDraggingRef.current = false;
				}, 100);
			}}
			onResizeStop={(_e, _direction, ref, _delta, position) => {
				const next = screenRectToRecordingPercent({
					x: position.x,
					y: position.y,
					width: ref.offsetWidth,
					height: ref.offsetHeight,
				});
				onPositionChange(annotation.id, next.position);
				onSizeChange(annotation.id, next.size);
			}}
			onClick={() => {
				if (isDraggingRef.current) return;
				onClick(annotation.id);
			}}
			bounds="parent"
			className={cn(
				"cursor-move transition-all",
				isSelected && "ring-2 ring-[#2563EB] ring-offset-2 ring-offset-transparent",
			)}
			style={{
				zIndex: isSelectedBoost ? zIndex + 1000 : zIndex, // Boost selected annotation to ensure it's on top
				pointerEvents: "auto",
				border: isSelected ? "2px solid rgba(37, 99, 235, 0.8)" : "none",
				backgroundColor: isSelected ? "rgba(37, 99, 235, 0.1)" : "transparent",
				boxShadow: isSelected ? "0 0 0 1px rgba(37, 99, 235, 0.35)" : "none",
				opacity: stage.alpha,
				transform: `translateY(${stage.translateY}px) scale(${stage.scale}) rotate(${stage.rotate}deg)`,
				transformOrigin: "center",
			}}
			enableResizing={isSelected}
			disableDragging={!isSelected || annotation.type === "drawing"}
			resizeHandleStyles={{
				topLeft: {
					width: "12px",
					height: "12px",
					backgroundColor: isSelected ? "white" : "transparent",
					border: isSelected ? "2px solid #2563EB" : "none",
					borderRadius: "50%",
					left: "-6px",
					top: "-6px",
					cursor: "nwse-resize",
				},
				topRight: {
					width: "12px",
					height: "12px",
					backgroundColor: isSelected ? "white" : "transparent",
					border: isSelected ? "2px solid #2563EB" : "none",
					borderRadius: "50%",
					right: "-6px",
					top: "-6px",
					cursor: "nesw-resize",
				},
				bottomLeft: {
					width: "12px",
					height: "12px",
					backgroundColor: isSelected ? "white" : "transparent",
					border: isSelected ? "2px solid #2563EB" : "none",
					borderRadius: "50%",
					left: "-6px",
					bottom: "-6px",
					cursor: "nesw-resize",
				},
				bottomRight: {
					width: "12px",
					height: "12px",
					backgroundColor: isSelected ? "white" : "transparent",
					border: isSelected ? "2px solid #2563EB" : "none",
					borderRadius: "50%",
					right: "-6px",
					bottom: "-6px",
					cursor: "nwse-resize",
				},
			}}
		>
			<div
				className={cn(
					"w-full h-full rounded-lg",
					annotation.type === "text" && "bg-transparent",
					annotation.type === "image" && "bg-transparent",
					annotation.type === "figure" && "bg-transparent",
					annotation.type === "drawing" && "bg-transparent",
					isSelected && "shadow-lg",
				)}
			>
				{renderContent()}
			</div>
		</Rnd>
	);
}
