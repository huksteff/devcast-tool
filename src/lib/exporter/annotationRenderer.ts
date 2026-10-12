import {
	type AnnotationRegion,
	type ArrowDirection,
	BASE_PREVIEW_WIDTH,
	BLUR_ANNOTATION_STRENGTH,
	getAnnotationStageState,
	getVisibleDrawingStrokes,
	getAnimatedGradientColors,
	DEFAULT_CLICK_HIGHLIGHT_DATA,
} from "@/components/video-editor/types";
import { resolveAnnotationAtTime, resolveAnnotationTransformAtTime } from "@/components/video-editor/annotationKeyframes";
import { compareAnnotationLayerOrder } from "@/components/video-editor/annotationLayerOrder";

export interface AnnotationRenderAssets {
	imageCache: Map<string, HTMLImageElement>;
}

export function getAnnotationContentScale(width: number): number {
	return Math.max(0.01, width / BASE_PREVIEW_WIDTH);
}

interface AnnotationSceneTransform {
	scale: number;
	x: number;
	y: number;
}

interface AnnotationCoordinateRect {
	x: number;
	y: number;
	width: number;
	height: number;
}

function transformAnnotationRect(
	rect: { x: number; y: number; width: number; height: number },
	sceneTransform?: AnnotationSceneTransform,
) {
	if (!sceneTransform) {
		return rect;
	}

	return {
		x: rect.x * sceneTransform.scale + sceneTransform.x,
		y: rect.y * sceneTransform.scale + sceneTransform.y,
		width: rect.width * sceneTransform.scale,
		height: rect.height * sceneTransform.scale,
	};
}

const annotationImagePromiseCache = new Map<string, Promise<HTMLImageElement | null>>();

let blurBufferCanvas: HTMLCanvasElement | null = null;
function getBlurBufferCanvas(): HTMLCanvasElement | null {
	if (typeof document === "undefined") return null;
	if (!blurBufferCanvas) {
		blurBufferCanvas = document.createElement("canvas");
	}
	return blurBufferCanvas;
}

function getAnnotationImageContent(annotation: AnnotationRegion): string | null {
	const source = annotation.imageContent || annotation.content;
	if (!source || !source.startsWith("data:image")) {
		return null;
	}

	return source;
}

function loadAnnotationImage(source: string): Promise<HTMLImageElement | null> {
	const cachedPromise = annotationImagePromiseCache.get(source);
	if (cachedPromise) {
		return cachedPromise;
	}

	const loadPromise = new Promise<HTMLImageElement | null>((resolve) => {
		const img = new Image();
		img.onload = () => resolve(img);
		img.onerror = () => {
			console.error("[AnnotationRenderer] Failed to load image annotation");
			resolve(null);
		};
		img.src = source;
	});

	annotationImagePromiseCache.set(source, loadPromise);
	return loadPromise;
}

export async function preloadAnnotationAssets(
	annotations: AnnotationRegion[] = [],
): Promise<AnnotationRenderAssets> {
	const uniqueSources = [
		...new Set(
			annotations
				.filter((annotation) => annotation.type === "image")
				.map((annotation) => getAnnotationImageContent(annotation))
				.filter((source): source is string => !!source),
		),
	];

	if (uniqueSources.length === 0) {
		return { imageCache: new Map() };
	}

	const loadedSources = await Promise.all(
		uniqueSources.map(async (source) => {
			const image = await loadAnnotationImage(source);
			return image ? ([source, image] as const) : null;
		}),
	);

	return {
		imageCache: new Map(
			loadedSources.filter((entry): entry is readonly [string, HTMLImageElement] => !!entry),
		),
	};
}

const ARROW_PATHS: Record<ArrowDirection, string[]> = {
	up: ["M 50 20 L 50 80", "M 50 20 L 35 35", "M 50 20 L 65 35"],
	down: ["M 50 20 L 50 80", "M 50 80 L 35 65", "M 50 80 L 65 65"],
	left: ["M 80 50 L 20 50", "M 20 50 L 35 35", "M 20 50 L 35 65"],
	right: ["M 20 50 L 80 50", "M 80 50 L 65 35", "M 80 50 L 65 65"],
	"up-right": ["M 25 75 L 75 25", "M 75 25 L 60 30", "M 75 25 L 70 40"],
	"up-left": ["M 75 75 L 25 25", "M 25 25 L 40 30", "M 25 25 L 30 40"],
	"down-right": ["M 25 25 L 75 75", "M 75 75 L 70 60", "M 75 75 L 60 70"],
	"down-left": ["M 75 25 L 25 75", "M 25 75 L 30 60", "M 25 75 L 40 70"],
};

function parseSvgPath(
	pathString: string,
	scaleX: number,
	scaleY: number,
): Array<{ cmd: string; args: number[] }> {
	const commands: Array<{ cmd: string; args: number[] }> = [];
	const parts = pathString.trim().split(/\s+/);

	let i = 0;
	while (i < parts.length) {
		const cmd = parts[i];
		if (cmd === "M" || cmd === "L") {
			const x = parseFloat(parts[i + 1]) * scaleX;
			const y = parseFloat(parts[i + 2]) * scaleY;
			commands.push({ cmd, args: [x, y] });
			i += 3;
		} else {
			i++;
		}
	}

	return commands;
}

function renderArrow(
	ctx: CanvasRenderingContext2D,
	direction: ArrowDirection,
	colors: string[],
	strokeWidth: number,
	opacity: number,
	glow: number,
	x: number,
	y: number,
	width: number,
	height: number,
	_scaleFactor: number,
) {
	const paths = ARROW_PATHS[direction];
	if (!paths) return;

	ctx.save();
	ctx.translate(x, y);

	const padding = 8 * _scaleFactor;
	const availableWidth = Math.max(0, width - padding * 2);
	const availableHeight = Math.max(0, height - padding * 2);

	const scale = Math.min(availableWidth / 100, availableHeight / 100);

	const offsetX = padding + (availableWidth - 100 * scale) / 2;
	const offsetY = padding + (availableHeight - 100 * scale) / 2;

	ctx.translate(offsetX, offsetY);
	ctx.globalAlpha *= opacity;

	ctx.shadowColor = glow > 0 ? colors[Math.floor(colors.length / 2)] : "rgba(0, 0, 0, 0.3)";
	ctx.shadowBlur = glow > 0 ? glow * _scaleFactor * 1.5 : 8 * scale;
	ctx.shadowOffsetX = 0;
	ctx.shadowOffsetY = glow > 0 ? 0 : 4 * scale;

	if (colors.length > 1) {
		const gradient = ctx.createLinearGradient(0, 0, 100 * scale, 0);
		colors.forEach((color, index) => gradient.addColorStop(index / (colors.length - 1), color));
		ctx.strokeStyle = gradient;
	} else {
		ctx.strokeStyle = colors[0];
	}
	ctx.lineWidth = strokeWidth * scale;
	ctx.lineCap = "round";
	ctx.lineJoin = "round";

	ctx.beginPath();

	for (const pathString of paths) {
		const commands = parseSvgPath(pathString, scale, scale);

		for (const { cmd, args } of commands) {
			if (cmd === "M") {
				ctx.moveTo(args[0], args[1]);
			} else if (cmd === "L") {
				ctx.lineTo(args[0], args[1]);
			}
		}
	}

	ctx.stroke();

	ctx.restore();
}

function renderFigure(
	ctx: CanvasRenderingContext2D,
	annotation: AnnotationRegion,
	x: number,
	y: number,
	width: number,
	height: number,
	scaleFactor: number,
	currentTimeMs: number,
) {
	const figure = annotation.figureData;
	if (!figure) return;
	const kind = figure.kind ?? "arrow";
	const opacity = Math.max(0, Math.min(1, figure.opacity ?? (kind === "highlight" ? 0.28 : 1)));
	const fillEnabled = figure.fillEnabled ?? kind === "highlight";
	const outlineEnabled = figure.outlineEnabled ?? kind !== "highlight";
	const fillGradient = figure.fillGradient ?? (kind === "highlight" ? figure.gradient : undefined);
	const gradientColors = getAnimatedGradientColors(figure.gradient, currentTimeMs, figure.color);
	const fillColors = getAnimatedGradientColors(fillGradient, currentTimeMs, figure.fillColor ?? figure.color);
	const glow = Math.max(
		outlineEnabled && figure.gradient?.enabled ? figure.gradient.glow : 0,
		fillEnabled && fillGradient?.enabled ? fillGradient.glow : 0,
	);
	if ((figure.kind ?? "arrow") === "arrow") {
		renderArrow(
			ctx,
			figure.arrowDirection,
			gradientColors,
			figure.strokeWidth,
			opacity,
			glow,
			x,
			y,
			width,
			height,
			scaleFactor,
		);
		return;
	}
	ctx.save();
	ctx.globalAlpha *= opacity;
	const strokePaint =
		gradientColors.length > 1
			? (() => {
					const gradient = ctx.createLinearGradient(x, y, x + width, y);
					gradientColors.forEach((color, index) =>
						gradient.addColorStop(index / (gradientColors.length - 1), color),
					);
					return gradient;
				})()
			: figure.color;
	const fillPaint = fillColors.length > 1
		? (() => {
			const gradient = ctx.createLinearGradient(x, y, x + width, y);
			fillColors.forEach((color, index) => gradient.addColorStop(index / (fillColors.length - 1), color));
			return gradient;
		})()
		: fillColors[0];
	ctx.strokeStyle = strokePaint;
	ctx.fillStyle = fillPaint;
	if (glow > 0) {
		ctx.shadowColor = fillEnabled && fillGradient?.enabled
			? fillColors[Math.floor(fillColors.length / 2)]
			: gradientColors[Math.floor(gradientColors.length / 2)];
		ctx.shadowBlur = glow * scaleFactor * 1.5;
	}
	ctx.lineWidth = Math.max(1, figure.strokeWidth * scaleFactor);
	ctx.lineCap = "round";
	ctx.lineJoin = "round";
	const inset = ctx.lineWidth / 2;
	if (kind === "highlight") {
		if (fillEnabled) ctx.fillRect(x, y, width, height);
		if (outlineEnabled) ctx.strokeRect(x + inset, y + inset, Math.max(0, width - inset * 2), Math.max(0, height - inset * 2));
	} else if (kind === "ellipse") {
		ctx.beginPath();
		ctx.ellipse(
			x + width / 2,
			y + height / 2,
			Math.max(0, width / 2 - inset),
			Math.max(0, height / 2 - inset),
			0,
			0,
			Math.PI * 2,
		);
		if (fillEnabled) ctx.fill();
		if (outlineEnabled) ctx.stroke();
	} else if (kind === "circle") {
		const radius = Math.max(0, Math.min(width, height) / 2 - inset);
		ctx.beginPath();
		ctx.arc(x + width / 2, y + height / 2, radius, 0, Math.PI * 2);
		if (fillEnabled) ctx.fill();
		if (outlineEnabled) ctx.stroke();
	} else if (kind === "line") {
		ctx.beginPath();
		ctx.moveTo(x + inset, y + height - inset);
		ctx.lineTo(x + width - inset, y + inset);
		if (outlineEnabled) ctx.stroke();
	} else {
		const rectX = x + inset;
		const rectY = y + inset;
		const rectWidth = Math.max(0, width - inset * 2);
		const rectHeight = Math.max(0, height - inset * 2);
		if (fillEnabled) ctx.fillRect(rectX, rectY, rectWidth, rectHeight);
		if (outlineEnabled) ctx.strokeRect(rectX, rectY, rectWidth, rectHeight);
	}
	ctx.restore();
}

function renderDrawing(
	ctx: CanvasRenderingContext2D,
	annotation: AnnotationRegion,
	x: number,
	y: number,
	width: number,
	height: number,
	scaleFactor: number,
	progress: number = 1,
	currentTimeMs: number = 0,
) {
	const drawing = annotation.drawingData;
	if (!drawing) return;
	const strokes = getVisibleDrawingStrokes(drawing, progress);
	if (!strokes.some((stroke) => stroke.length > 0)) return;
	ctx.save();
	const gradientColors = getAnimatedGradientColors(
		drawing.gradient,
		currentTimeMs,
		drawing.color,
	);
	if (gradientColors.length > 1) {
		const gradient = ctx.createLinearGradient(x, y, x + width, y);
		gradientColors.forEach((color, index) =>
			gradient.addColorStop(index / (gradientColors.length - 1), color),
		);
		ctx.strokeStyle = gradient;
		ctx.shadowColor = gradientColors[0];
		ctx.shadowBlur = Math.max(0, drawing.gradient?.glow || 0) * scaleFactor;
	} else ctx.strokeStyle = drawing.color;
	ctx.lineWidth = Math.max(1, drawing.strokeWidth * scaleFactor);
	ctx.lineCap = "round";
	ctx.lineJoin = "round";
	for (const stroke of strokes) {
		if (stroke.length === 0) continue;
		ctx.beginPath();
		stroke.forEach((point, index) => {
			const px = x + (point.x / 100) * width;
			const py = y + (point.y / 100) * height;
			if (index === 0) ctx.moveTo(px, py);
			else ctx.lineTo(px, py);
		});
		if (stroke.length === 1) {
			ctx.lineTo(
				x + (stroke[0].x / 100) * width + 0.01,
				y + (stroke[0].y / 100) * height + 0.01,
			);
		}
		ctx.stroke();
	}
	if (
		drawing.showEraserInDrawOut &&
		annotation.stages?.outEnabled &&
		annotation.stages.outTransition === "draw-out" &&
		progress < 1
	) {
		const lastStroke = strokes[strokes.length - 1];
		const point = lastStroke?.[lastStroke.length - 1];
		if (point) {
			const radius = Math.max(2, ((drawing.eraserSize ?? 12) * scaleFactor) / 2);
			ctx.strokeStyle = "#FFFFFF";
			ctx.lineWidth = Math.max(1, scaleFactor * 2);
			ctx.fillStyle = "rgba(0, 0, 0, 0.2)";
			ctx.beginPath();
			ctx.arc(x + (point.x / 100) * width, y + (point.y / 100) * height, radius, 0, Math.PI * 2);
			ctx.fill();
			ctx.stroke();
		}
	}
	ctx.restore();
}

function renderClickHighlight(
	ctx: CanvasRenderingContext2D,
	annotation: AnnotationRegion,
	x: number,
	y: number,
	width: number,
	height: number,
	currentTimeMs: number,
	scaleFactor: number,
) {
	const data = { ...DEFAULT_CLICK_HIGHLIGHT_DATA, ...annotation.clickData };
	const age = Math.max(0, currentTimeMs - annotation.startMs);
	const progress = Math.min(1, age / Math.max(1, data.durationMs));
	if (progress <= 0) return;
	const fade = (1 - progress) ** 1.35;
	const centerX = x + width / 2;
	const centerY = y + height / 2;
	const targetRadius = Math.min(data.radius * scaleFactor, Math.min(width, height) * 0.48);
	const expand = 1 - (1 - progress) ** 3;
	const radius = targetRadius * expand;
	const colorWithAlpha = (color: string, alpha: number) => {
		const normalized = color.replace("#", "");
		if (/^[0-9a-fA-F]{6}$/.test(normalized)) {
			const value = Number.parseInt(normalized, 16);
			return `rgba(${(value >> 16) & 255}, ${(value >> 8) & 255}, ${value & 255}, ${alpha})`;
		}
		return color;
	};
	ctx.save();
	ctx.globalAlpha *= fade;
	// A radial bloom intentionally has no hard outline. This makes it read as a
	// light flash rather than a second circular annotation.
	const glow = ctx.createRadialGradient(centerX, centerY, 0, centerX, centerY, radius);
	glow.addColorStop(0, colorWithAlpha(data.color, 0.62));
	glow.addColorStop(0.42, colorWithAlpha(data.color, 0.2));
	glow.addColorStop(1, colorWithAlpha(data.color, 0));
	ctx.fillStyle = glow;
	ctx.beginPath();
	ctx.arc(centerX, centerY, radius, 0, Math.PI * 2);
	ctx.fill();
	ctx.globalAlpha = fade;
	for (let index = 0; index < data.particleCount; index++) {
		const angle = index * 2.399963229728653;
		const distribution = Math.sqrt((index + 0.5) / Math.max(1, data.particleCount));
		const distance = targetRadius * distribution * (0.82 + expand * 0.18);
		ctx.fillStyle = data.color;
		ctx.beginPath();
		ctx.arc(
			centerX + Math.cos(angle) * distance,
			centerY + Math.sin(angle) * distance,
			Math.max(1, targetRadius * (0.018 + (index % 4) * 0.005)) * expand,
			0,
			Math.PI * 2,
		);
		ctx.fill();
	}
	ctx.restore();
}

function renderText(
	ctx: CanvasRenderingContext2D,
	annotation: AnnotationRegion,
	x: number,
	y: number,
	width: number,
	height: number,
	scaleFactor: number,
	content: string = annotation.content,
) {
	const style = annotation.style;

	ctx.save();

	ctx.beginPath();
	ctx.rect(x, y, width, height);
	ctx.clip();

	const fontWeight = style.fontWeight === "bold" ? "bold" : "normal";
	const fontStyle = style.fontStyle === "italic" ? "italic" : "normal";
	const scaledFontSize = style.fontSize * scaleFactor;
	ctx.font = `${fontStyle} ${fontWeight} ${scaledFontSize}px ${style.fontFamily}`;
	ctx.textBaseline = "middle";

	const containerPadding = 8 * scaleFactor;

	let textX = x;
	const textY = y + height / 2;

	if (style.textAlign === "center") {
		textX = x + width / 2;
		ctx.textAlign = "center";
	} else if (style.textAlign === "right") {
		textX = x + width - containerPadding;
		ctx.textAlign = "right";
	} else {
		textX = x + containerPadding;
		ctx.textAlign = "left";
	}

	const availableWidth = width - containerPadding * 2;
	const rawLines = content.split("\n");
	const lines: string[] = [];
	for (const rawLine of rawLines) {
		if (!rawLine) {
			lines.push("");
			continue;
		}
		const words = rawLine.split(/(\s+)/);
		let current = "";
		for (const word of words) {
			const test = current + word;
			if (current && ctx.measureText(test).width > availableWidth) {
				lines.push(current);
				current = word.trimStart();
			} else {
				current = test;
			}
		}
		if (current) lines.push(current);
	}
	const lineHeight = scaledFontSize * 1.4;

	const startY = textY - ((lines.length - 1) * lineHeight) / 2;

	lines.forEach((line, index) => {
		const currentY = startY + index * lineHeight;

		if (style.backgroundColor && style.backgroundColor !== "transparent") {
			const metrics = ctx.measureText(line);
			const verticalPadding = scaledFontSize * 0.1;
			const horizontalPadding = scaledFontSize * 0.2;
			const borderRadius = 4 * scaleFactor;

			let bgX = textX - horizontalPadding;
			const bgWidth = metrics.width + horizontalPadding * 2;

			const contentHeight = scaledFontSize * 1.4;
			const bgHeight = contentHeight + verticalPadding * 2;
			const bgY = currentY - bgHeight / 2;

			if (style.textAlign === "center") {
				bgX = textX - bgWidth / 2;
			} else if (style.textAlign === "right") {
				bgX = textX - bgWidth;
			}

			ctx.fillStyle = style.backgroundColor;
			ctx.beginPath();
			ctx.roundRect(bgX, bgY, bgWidth, bgHeight, borderRadius);
			ctx.fill();
		}

		ctx.fillStyle = style.color;
		ctx.fillText(line, textX, currentY);

		if (style.textDecoration === "underline") {
			const metrics = ctx.measureText(line);
			let underlineX = textX;
			const underlineY = currentY + scaledFontSize * 0.15;

			if (style.textAlign === "center") {
				underlineX = textX - metrics.width / 2;
			} else if (style.textAlign === "right") {
				underlineX = textX - metrics.width;
			}

			ctx.strokeStyle = style.color;
			ctx.lineWidth = Math.max(1, scaledFontSize / 16);
			ctx.beginPath();
			ctx.moveTo(underlineX, underlineY);
			ctx.lineTo(underlineX + metrics.width, underlineY);
			ctx.stroke();
		}
	});

	ctx.restore();
}

async function renderImage(
	ctx: CanvasRenderingContext2D,
	annotation: AnnotationRegion,
	x: number,
	y: number,
	width: number,
	height: number,
	assets?: AnnotationRenderAssets,
): Promise<void> {
	const source = getAnnotationImageContent(annotation);
	if (!source) {
		return;
	}

	const img = assets?.imageCache.get(source) ?? (await loadAnnotationImage(source));
	if (!img) {
		return;
	}

	const imgAspect = img.width / img.height;
	const boxAspect = width / height;

	let drawWidth = width;
	let drawHeight = height;
	let drawX = x;
	let drawY = y;

	if (imgAspect > boxAspect) {
		drawHeight = width / imgAspect;
		drawY = y + (height - drawHeight) / 2;
	} else {
		drawWidth = height * imgAspect;
		drawX = x + (width - drawWidth) / 2;
	}

	ctx.drawImage(img, drawX, drawY, drawWidth, drawHeight);
}

export async function renderAnnotations(
	ctx: CanvasRenderingContext2D,
	annotations: AnnotationRegion[],
	canvasWidth: number,
	canvasHeight: number,
	currentTimeMs: number,
	scaleFactor: number = 1.0,
	assets?: AnnotationRenderAssets,
	sceneTransform?: AnnotationSceneTransform,
	coordinateRect?: AnnotationCoordinateRect,
): Promise<void> {
	const activeAnnotations = annotations.filter(
		(ann) => currentTimeMs >= ann.startMs && currentTimeMs <= ann.endMs,
	);

	const sortedAnnotations = [...activeAnnotations].sort(compareAnnotationLayerOrder);
	const annotationRect = coordinateRect ?? {
		x: 0,
		y: 0,
		width: canvasWidth,
		height: canvasHeight,
	};
	const contentScaleFactor = coordinateRect
		? getAnnotationContentScale(annotationRect.width)
		: scaleFactor;

	for (const sourceAnnotation of sortedAnnotations) {
		const annotation = resolveAnnotationAtTime(sourceAnnotation, currentTimeMs);
		const transform = resolveAnnotationTransformAtTime(annotation, currentTimeMs);
		const { position, size } = transform;
		const stage = getAnnotationStageState(annotation, currentTimeMs);
		ctx.save();
		ctx.globalAlpha = stage.alpha;
		const rect = transformAnnotationRect(
			{
				x: annotationRect.x + (position.x / 100) * annotationRect.width,
				y: annotationRect.y + (position.y / 100) * annotationRect.height,
				width: (size.width / 100) * annotationRect.width,
				height: (size.height / 100) * annotationRect.height,
			},
			sceneTransform,
		);
		const { x, y, width, height } = rect;
		const effectiveScaleFactor = contentScaleFactor * (sceneTransform?.scale ?? 1);
		ctx.translate(
			x + width / 2 + stage.translateX * effectiveScaleFactor,
			y + height / 2 + stage.translateY * effectiveScaleFactor,
		);
		ctx.scale(stage.scale, stage.scale);
		ctx.rotate((stage.rotate * Math.PI) / 180);
		ctx.translate(-(x + width / 2), -(y + height / 2));

		switch (annotation.type) {
			case "text":
				renderText(
					ctx,
					annotation,
					x,
					y,
					width,
					height,
					effectiveScaleFactor,
					annotation.content.slice(
						0,
						Math.ceil(annotation.content.length * stage.textProgress),
					),
				);
				break;

			case "image":
				await renderImage(ctx, annotation, x, y, width, height, assets);
				break;

			case "figure":
				renderFigure(
					ctx,
					annotation,
					x,
					y,
					width,
					height,
					effectiveScaleFactor,
					currentTimeMs,
				);
				break;

			case "drawing":
				renderDrawing(
					ctx,
					annotation,
					x,
					y,
					width,
					height,
					effectiveScaleFactor,
					stage.drawingProgress,
					currentTimeMs,
				);
				break;

			case "click":
				renderClickHighlight(
					ctx,
					annotation,
					x,
					y,
					width,
					height,
					currentTimeMs,
					effectiveScaleFactor,
				);
				break;

			case "blur": {
				const blurStrength =
					(annotation.blurIntensity ?? BLUR_ANNOTATION_STRENGTH) * effectiveScaleFactor;
				const padding = Math.ceil(blurStrength * 2);

				ctx.save();

				ctx.beginPath();
				const borderRadius = (annotation.style.borderRadius ?? 0) * effectiveScaleFactor;
				ctx.roundRect(x, y, width, height, borderRadius);
				ctx.clip();

				const sx = Math.max(0, x - padding);
				const sy = Math.max(0, y - padding);
				const sw = Math.min(canvasWidth - sx, width + padding * 2);
				const sh = Math.min(canvasHeight - sy, height + padding * 2);

				if (sw > 0 && sh > 0) {
					const buffer = getBlurBufferCanvas();
					if (buffer) {
						buffer.width = sw;
						buffer.height = sh;
						const bCtx = buffer.getContext("2d");
						if (bCtx) {
							bCtx.drawImage(ctx.canvas, sx, sy, sw, sh, 0, 0, sw, sh);

							ctx.filter = `blur(${blurStrength}px)`;
							ctx.drawImage(buffer, sx, sy);

							if (annotation.blurColor && annotation.blurColor !== "transparent") {
								ctx.filter = "none";
								ctx.fillStyle = annotation.blurColor;
								ctx.fillRect(x, y, width, height);
							}
						}
					}
				}

				ctx.restore();
				break;
			}
		}
		ctx.restore();
	}
}

export async function renderAnnotationToCanvas(
	annotation: AnnotationRegion,
	width: number,
	height: number,
	scaleFactor: number = 1.0,
	assets?: AnnotationRenderAssets,
): Promise<HTMLCanvasElement | null> {
	const canvasWidth = Math.max(1, Math.ceil(width));
	const canvasHeight = Math.max(1, Math.ceil(height));
	const canvas = document.createElement("canvas");
	canvas.width = canvasWidth;
	canvas.height = canvasHeight;

	const ctx = canvas.getContext("2d");
	if (!ctx) {
		return null;
	}

	ctx.imageSmoothingEnabled = true;
	ctx.imageSmoothingQuality = "high";

	switch (annotation.type) {
		case "text":
			renderText(ctx, annotation, 0, 0, canvasWidth, canvasHeight, scaleFactor);
			break;

		case "image":
			await renderImage(ctx, annotation, 0, 0, canvasWidth, canvasHeight, assets);
			break;

		case "figure":
			if (!annotation.figureData) {
				return null;
			}

			renderFigure(
				ctx,
				annotation,
				0,
				0,
				canvasWidth,
				canvasHeight,
				scaleFactor,
				annotation.startMs,
			);
			break;
		case "drawing":
			renderDrawing(
				ctx,
				annotation,
				0,
				0,
				canvasWidth,
				canvasHeight,
				scaleFactor,
				1,
				annotation.startMs,
			);
			break;
		case "click":
			renderClickHighlight(
				ctx,
				annotation,
				0,
				0,
				canvasWidth,
				canvasHeight,
				annotation.startMs,
				scaleFactor,
			);
			break;
		case "blur":
		case "cursor-callout":
			// Blur annotations must sample already-rendered scene pixels,
			// so they cannot be rasterized as standalone sprites.
			return null;
	}

	return canvas;
}
