import { useRef, useState } from "react";
import { Rnd } from "react-rnd";
import { cn } from "@/lib/utils";
import { resolveAnnotationTransformAtTime } from "./annotationKeyframes";
import { getArrowComponent } from "./ArrowSvgs";
import {
	type AnnotationRegion,
	type DrawingData,
	BASE_PREVIEW_WIDTH,
	BLUR_ANNOTATION_STRENGTH,
	getAnnotationStageState,
	getVisibleDrawingStrokes,
	eraseDrawingAtPoint,
	getAnimatedGradientColors,
	DEFAULT_CLICK_HIGHLIGHT_DATA,
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
	onTransformChange?: (
		id: string,
		transform: { position: { x: number; y: number }; size: { width: number; height: number } },
	) => void;
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
	onTransformChange,
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
	const resolvedTransform = resolveAnnotationTransformAtTime(annotation, currentTimeMs);
	const resolvedPosition = resolvedTransform.position;
	const sceneX = safeRecordingRect.x + (resolvedPosition.x / 100) * safeRecordingRect.width;
	const sceneY = safeRecordingRect.y + (resolvedPosition.y / 100) * safeRecordingRect.height;
	const sceneWidth = (resolvedTransform.size.width / 100) * safeRecordingRect.width;
	const sceneHeight = (resolvedTransform.size.height / 100) * safeRecordingRect.height;
	const x = sceneX * sceneTransform.scale + sceneTransform.x;
	const y = sceneY * sceneTransform.scale + sceneTransform.y;
	const width = sceneWidth * sceneTransform.scale;
	const height = sceneHeight * sceneTransform.scale;
	const sizeScale = safeRecordingRect.width / BASE_PREVIEW_WIDTH;
	const blurScaleFactor = sizeScale * sceneTransform.scale;
	const stage = getAnnotationStageState(annotation, currentTimeMs);
	const gradientId = `annotation-gradient-${annotation.id}`;

	const isDraggingRef = useRef(false);
	const drawingRef = useRef<DrawingData | null>(null);
	const activePointerRef = useRef<number | null>(null);
	const panRef = useRef<{
		pointerId: number;
		clientX: number;
		clientY: number;
		position: { x: number; y: number };
	} | null>(null);
	const [drawingCursor, setDrawingCursor] = useState<{
		x: number;
		y: number;
		erasing: boolean;
	} | null>(null);

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
		const colors = getAnimatedGradientColors(
			annotation.figureData?.gradient,
			currentTimeMs,
			annotation.figureData?.color || "#2563EB",
		);
		const color = colors.length > 1 ? `url(#${gradientId})` : colors[0];
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
							{annotation.content.slice(
								0,
								Math.ceil(annotation.content.length * stage.textProgress),
							)}
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
				const figureColors = getAnimatedGradientColors(
					annotation.figureData.gradient,
					currentTimeMs,
					annotation.figureData.color,
				);
				const figureColor =
					figureColors.length > 1 ? `url(#${gradientId})` : figureColors[0];
				const gradientDefinition =
					figureColors.length > 1 ? (
						<defs>
							<linearGradient id={gradientId} x1="0%" y1="0%" x2="100%" y2="0%">
								{figureColors.map((color, index) => (
									<stop
										key={`${color}-${index}`}
										offset={`${(index / (figureColors.length - 1)) * 100}%`}
										stopColor={color}
									/>
								))}
							</linearGradient>
						</defs>
					) : null;
				if ((annotation.figureData.kind ?? "arrow") !== "arrow") {
					const kind = annotation.figureData.kind ?? "rectangle";
					const common = {
						stroke: figureColor,
						strokeWidth: annotation.figureData.strokeWidth,
						vectorEffect: "non-scaling-stroke" as const,
						fill: "none",
					};
					return (
						<svg
							className="h-full w-full"
							viewBox="0 0 100 100"
							preserveAspectRatio={kind === "circle" ? "xMidYMid meet" : "none"}
						>
							{gradientDefinition}
							{kind === "ellipse" && (
								<ellipse cx="50" cy="50" rx="47" ry="47" {...common} />
							)}
							{kind === "circle" && <circle cx="50" cy="50" r="47" {...common} />}
							{kind === "line" && (
								<line
									x1="3"
									y1="97"
									x2="97"
									y2="3"
									{...common}
									strokeLinecap="round"
								/>
							)}
							{kind === "highlight" && (
								<rect
									width="100"
									height="100"
									fill={annotation.figureData.color}
									opacity="0.28"
								/>
							)}
							{kind === "rectangle" && (
								<rect x="3" y="3" width="94" height="94" {...common} />
							)}
						</svg>
					);
				}

				return (
					<div
						className="w-full h-full flex items-center justify-center"
						style={{ padding: `${8 * sizeScale}px` }}
					>
						<svg className="absolute h-0 w-0">
							<defs>
								<linearGradient id={gradientId} x1="0%" y1="0%" x2="100%" y2="0%">
									{figureColors.map((color, index) => (
										<stop
											key={`${color}-${index}`}
											offset={`${(index / (figureColors.length - 1)) * 100}%`}
											stopColor={color}
										/>
									))}
								</linearGradient>
							</defs>
						</svg>
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
				const strokes = drawing?.strokes?.length
					? drawing.strokes
					: drawing?.points?.length
						? [drawing.points]
						: [];
				// Keep input responsive while editing the current path. At playback the
				// stage transition determines exactly how much of the drawing is shown.
				const visibleStrokes =
					isSelected && activePointerRef.current !== null
						? strokes
						: getVisibleDrawingStrokes(drawing, stage.drawingProgress);
				const getPoint = (event: React.PointerEvent<SVGSVGElement>) => {
					const rect = event.currentTarget.getBoundingClientRect();
					return {
						x: Math.max(
							0,
							Math.min(
								100,
								((event.clientX - rect.left) / Math.max(1, rect.width)) * 100,
							),
						),
						y: Math.max(
							0,
							Math.min(
								100,
								((event.clientY - rect.top) / Math.max(1, rect.height)) * 100,
							),
						),
					};
				};
				const updateStroke = (event: React.PointerEvent<SVGSVGElement>, start: boolean) => {
					if (!isSelected || !onDrawingChange) return;
					event.stopPropagation();
					const point = getPoint(event);
					const previousStrokes = drawingRef.current?.strokes ?? strokes;
					const erasing = (drawing?.tool ?? "pencil") === "eraser";
					if (erasing) {
						const next = eraseDrawingAtPoint(
							{
								...(drawingRef.current ??
									drawing ?? { color: "#2563EB", strokeWidth: 4, points: [] }),
								strokes: previousStrokes,
							},
							point,
						((drawing?.eraserSize ?? 12) / 2 /
							Math.max(
								1,
								Math.min(
									event.currentTarget.getBoundingClientRect().width,
									event.currentTarget.getBoundingClientRect().height,
								),
							)) *
							100,
						);
						drawingRef.current = next;
						onDrawingChange(annotation.id, next);
						return;
					}
					const nextStrokes = start
						? [...previousStrokes, [point]]
						: previousStrokes.length
							? [
									...previousStrokes.slice(0, -1),
									event.shiftKey
										? [previousStrokes[previousStrokes.length - 1][0], point]
										: [...previousStrokes[previousStrokes.length - 1], point],
								]
							: [[point]];
					const next: DrawingData = {
						color: drawing?.color ?? "#2563EB",
						strokeWidth: drawing?.strokeWidth ?? 4,
						tool: drawing?.tool ?? "pencil",
						eraserSize: drawing?.eraserSize ?? 12,
						// Keep points for backwards-compatible consumers; new rendering uses strokes.
						points: nextStrokes.flat(),
						strokes: nextStrokes,
					};
					drawingRef.current = next;
					onDrawingChange(annotation.id, next);
				};
				const drawingColors = getAnimatedGradientColors(
					drawing?.gradient,
					currentTimeMs,
					drawing?.color ?? "#2563EB",
				);
				const drawingColor =
					drawingColors.length > 1 ? `url(#${gradientId})` : drawingColors[0];
				const animationEraserPoint =
					drawing?.showEraserInDrawOut &&
					annotation.stages?.outEnabled &&
					annotation.stages.outTransition === "draw-out" &&
					stage.drawingProgress < 1
						? visibleStrokes.length
							? visibleStrokes[visibleStrokes.length - 1][
									visibleStrokes[visibleStrokes.length - 1].length - 1
								]
							: undefined
						: undefined;
				return (
					<div className="relative h-full w-full">
						<svg
							className={
								isSelected
									? "h-full w-full touch-none cursor-none"
									: "h-full w-full"
							}
							viewBox="0 0 100 100"
							preserveAspectRatio="none"
							onPointerDown={(event) => {
								if (!isSelected || !onDrawingChange) return;
								event.stopPropagation();
								if (event.button === 2) {
									panRef.current = {
										pointerId: event.pointerId,
										clientX: event.clientX,
										clientY: event.clientY,
										position: { ...annotation.position },
									};
									event.currentTarget.setPointerCapture(event.pointerId);
									return;
								}
								if (event.button !== 0) return;
								activePointerRef.current = event.pointerId;
								event.currentTarget.setPointerCapture(event.pointerId);
								updateStroke(event, true);
							}}
							onPointerMove={(event) => {
								const point = getPoint(event);
								setDrawingCursor({
									x: point.x,
									y: point.y,
									erasing: (drawing?.tool ?? "pencil") === "eraser",
								});
								if (panRef.current?.pointerId === event.pointerId) {
									const scaleX = Math.max(
										1,
										safeRecordingRect.width * sceneTransform.scale,
									);
									const scaleY = Math.max(
										1,
										safeRecordingRect.height * sceneTransform.scale,
									);
									onPositionChange(annotation.id, {
										x:
											panRef.current.position.x +
											((event.clientX - panRef.current.clientX) / scaleX) *
												100,
										y:
											panRef.current.position.y +
											((event.clientY - panRef.current.clientY) / scaleY) *
												100,
									});
									return;
								}
								if (
									activePointerRef.current === event.pointerId &&
									event.currentTarget.hasPointerCapture(event.pointerId)
								)
									updateStroke(event, false);
							}}
							onPointerUp={(event) => {
								if (panRef.current?.pointerId === event.pointerId) {
									panRef.current = null;
									event.currentTarget.releasePointerCapture(event.pointerId);
									return;
								}
								if (activePointerRef.current !== event.pointerId) return;
								drawingRef.current = null;
								activePointerRef.current = null;
								event.currentTarget.releasePointerCapture(event.pointerId);
							}}
							onPointerCancel={() => {
								drawingRef.current = null;
								activePointerRef.current = null;
								panRef.current = null;
							}}
							onPointerLeave={() => setDrawingCursor(null)}
							onContextMenu={(event) => {
								event.preventDefault();
							}}
						>
							{drawingColors.length > 1 && (
								<defs>
									<linearGradient
										id={gradientId}
										x1="0%"
										y1="0%"
										x2="100%"
										y2="0%"
									>
										{drawingColors.map((color, index) => (
											<stop
												key={`${color}-${index}`}
												offset={`${(index / (drawingColors.length - 1)) * 100}%`}
												stopColor={color}
											/>
										))}
									</linearGradient>
								</defs>
							)}
							{visibleStrokes.map((stroke, index) => {
								const points = stroke
									.map((point) => `${point.x},${point.y}`)
									.join(" ");
								return stroke.length === 1 ? (
									<line
										key={index}
										x1={stroke[0].x}
										y1={stroke[0].y}
										x2={stroke[0].x + 0.001}
										y2={stroke[0].y}
										stroke={drawingColor}
										strokeWidth={drawing?.strokeWidth ?? 4}
										vectorEffect="non-scaling-stroke"
										strokeLinecap="round"
									/>
								) : (
									<polyline
										key={index}
										points={points}
										fill="none"
										stroke={drawingColor}
										strokeWidth={drawing?.strokeWidth ?? 4}
										vectorEffect="non-scaling-stroke"
										strokeLinecap="round"
										strokeLinejoin="round"
										style={
											drawing?.gradient?.enabled
												? {
														filter: `drop-shadow(0 0 ${(drawing.gradient.glow ?? 14) * sizeScale}px ${drawingColors[0]})`,
													}
												: undefined
										}
									/>
								);
							})}
						</svg>
						{isSelected && drawingCursor && (
							<div
								className="pointer-events-none absolute rounded-full border border-white bg-black/10 shadow-sm"
								style={{
									left: `${drawingCursor.x}%`,
									top: `${drawingCursor.y}%`,
									width: drawingCursor.erasing
										? `${drawing?.eraserSize ?? 12}px`
										: `${drawing?.strokeWidth ?? 4}px`,
									height: drawingCursor.erasing
										? `${drawing?.eraserSize ?? 12}px`
										: `${drawing?.strokeWidth ?? 4}px`,
									transform: "translate(-50%, -50%)",
								}}
							/>
						)}
						{animationEraserPoint && (
							<div
								className="pointer-events-none absolute rounded-full border-2 border-white bg-black/20 shadow-md"
								style={{
									left: `${animationEraserPoint.x}%`,
									top: `${animationEraserPoint.y}%`,
									width: `${drawing?.eraserSize ?? 12}px`,
									height: `${drawing?.eraserSize ?? 12}px`,
									transform: "translate(-50%, -50%)",
								}}
							/>
						)}
					</div>
				);
			}

			case "click": {
				const data = { ...DEFAULT_CLICK_HIGHLIGHT_DATA, ...annotation.clickData };
				const progress = Math.min(
					1,
					Math.max(0, currentTimeMs - annotation.startMs) / Math.max(1, data.durationMs),
				);
				// The highlight is a soft flash, not a stroked ring.  Starting at zero is
				// important here: a large annotation should not pop in at an already huge size.
				const fade = (1 - progress) ** 1.35;
				const expand = 1 - (1 - progress) ** 3;
				const targetRadius = Math.min(
					data.radius * sizeScale * sceneTransform.scale,
					Math.min(width, height) * 0.48,
				);
				const radius = targetRadius * expand;
				const gradientId = `click-highlight-${annotation.id.replace(/[^a-zA-Z0-9_-]/g, "")}`;
				return (
					<svg
						className="h-full w-full overflow-visible"
						viewBox={`0 0 ${Math.max(1, width)} ${Math.max(1, height)}`}
						style={{ opacity: progress > 0 ? fade : 0 }}
					>
						<defs>
							<radialGradient id={gradientId}>
								<stop offset="0%" stopColor={data.color} stopOpacity="0.62" />
								<stop offset="42%" stopColor={data.color} stopOpacity="0.2" />
								<stop offset="100%" stopColor={data.color} stopOpacity="0" />
							</radialGradient>
						</defs>
						<circle
							cx={width / 2}
							cy={height / 2}
							r={radius}
							fill={`url(#${gradientId})`}
							style={{ filter: `drop-shadow(0 0 ${Math.max(2, radius * 0.26)}px ${data.color})` }}
						/>
						{Array.from({ length: data.particleCount }, (_, index) => {
							const angle = index * 2.399963229728653;
							const distribution = Math.sqrt((index + 0.5) / Math.max(1, data.particleCount));
							// Particles are distributed throughout the flash and drift only a little.
							const distance = targetRadius * distribution * (0.82 + expand * 0.18);
							return (
								<circle
									key={index}
									cx={width / 2 + Math.cos(angle) * distance}
									cy={height / 2 + Math.sin(angle) * distance}
									r={Math.max(1, targetRadius * (0.018 + (index % 4) * 0.005)) * expand}
									fill={data.color}
									opacity={0.5 + (index % 4) * 0.1}
								/>
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
				const next = screenRectToRecordingPercent({
					x: d.x,
					y: d.y,
					width,
					height,
				});
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
				if (onTransformChange) {
					onTransformChange(annotation.id, next);
				} else {
					onPositionChange(annotation.id, next.position);
					onSizeChange(annotation.id, next.size);
				}
			}}
			onClick={() => {
				if (isDraggingRef.current) return;
				onClick(annotation.id);
			}}
			bounds="parent"
			className={cn(
				"cursor-move",
				isSelected && "ring-2 ring-[#2563EB] ring-offset-2 ring-offset-transparent",
			)}
			style={{
				zIndex: isSelectedBoost ? zIndex + 1000 : zIndex, // Boost selected annotation to ensure it's on top
				pointerEvents: "auto",
				border: isSelected ? "2px solid rgba(37, 99, 235, 0.8)" : "none",
				backgroundColor: isSelected ? "rgba(37, 99, 235, 0.1)" : "transparent",
				boxShadow: isSelected ? "0 0 0 1px rgba(37, 99, 235, 0.35)" : "none",
				opacity: stage.alpha,
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
				style={{
					transform: `translate(${stage.translateX * blurScaleFactor}px, ${stage.translateY * blurScaleFactor}px) scale(${stage.scale}) rotate(${stage.rotate}deg)`,
					transformOrigin: "center",
					willChange: "transform",
				}}
			>
				{renderContent()}
			</div>
		</Rnd>
	);
}
