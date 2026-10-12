import { ToggleButton } from "@heroui/react";
import { Card } from "@heroui/react";
import { TextArea } from "@/components/ui/input";
import {
	AlignCenterHorizontal as AlignCenter,
	AlignLeft,
	AlignRight,
	TextB as Bold,
	ImageSquare as ImageIcon,
	Info,
	TextItalic as Italic,
	BoundingBox as SquareDashed,
	PencilSimple as Pencil,
	Eraser,
	Trash as Trash2,
	Circle,
	ChatText,
	TextT as Type,
	TextUnderline as Underline,
	UploadSimple as Upload,
} from "@phosphor-icons/react";
import { ColorControl, ColorPalette } from "@/components/ui/color-picker";
import { useEffect, useMemo, useRef, useState } from "react";
import { toast } from "@/components/ui/toast";
import { Button } from "@/components/ui/button";

import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import { Slider } from "@/components/ui/slider";
import { ChoiceGroup, ChoiceItem } from "@/components/ui/choice-group";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { type CustomFont, getCustomFonts } from "@/lib/customFonts";
import { cn } from "@/lib/utils";
import { useScopedT } from "../../contexts/I18nContext";
import { AddCustomFontDialog } from "./AddCustomFontDialog";
import { getArrowComponent } from "./ArrowSvgs";
import {
	type AnnotationRegion,
	type AnnotationType,
	type AnnotationStages,
	type AnnotationTransition,
	ArrowDirection,
	DrawingData,
	FigureData,
	ClickHighlightData,
	DEFAULT_ANNOTATION_GRADIENT,
	DEFAULT_CURSOR_CALLOUT_GRADIENT,
	DEFAULT_CLICK_HIGHLIGHT_DATA,
	DEFAULT_ANNOTATION_STAGES,
} from "./types";

interface AnnotationSettingsPanelProps {
	annotation: AnnotationRegion;
	onContentChange: (content: string) => void;
	onTypeChange: (type: AnnotationType) => void;
	onStyleChange: (style: Partial<AnnotationRegion["style"]>) => void;
	onFigureDataChange?: (figureData: FigureData) => void;
	onDrawingDataChange?: (drawingData: DrawingData) => void;
	onBlurIntensityChange?: (intensity: number) => void;
	onBlurColorChange?: (color: string) => void;
	onClickDataChange?: (data: ClickHighlightData) => void;
	onCalloutEventChange?: (data: NonNullable<AnnotationRegion["calloutEvent"]>) => void;
	onStagesChange?: (stages: Partial<AnnotationStages>) => void;
}

export const FONT_FAMILY_VALUES = [
	{
		value: "system-ui, -apple-system, sans-serif",
		labelKey: "fontStyles.classic",
	},
	{ value: "Georgia, serif", labelKey: "fontStyles.editor" },
	{ value: "Impact, Arial Black, sans-serif", labelKey: "fontStyles.strong" },
	{ value: "Courier New, monospace", labelKey: "fontStyles.typewriter" },
	{ value: "Brush Script MT, cursive", labelKey: "fontStyles.deco" },
	{ value: "Arial, sans-serif", labelKey: "fontStyles.simple" },
	{ value: "Verdana, sans-serif", labelKey: "fontStyles.modern" },
	{ value: "Trebuchet MS, sans-serif", labelKey: "fontStyles.clean" },
];

export const FONT_SIZES = [12, 14, 16, 18, 20, 24, 28, 32, 36, 40, 48, 56, 64, 72, 80, 96, 128];

type FigureKind = NonNullable<FigureData["kind"]>;

function FigureKindPreview({ kind }: { kind: FigureKind }) {
	return (
		<svg
			aria-hidden="true"
			viewBox="0 0 48 28"
			className="h-7 w-12 overflow-visible"
			fill="none"
			stroke="currentColor"
			strokeWidth="2.4"
			strokeLinecap="round"
			strokeLinejoin="round"
		>
			{kind === "arrow" && (
				<>
					<path d="M7 14h31" />
					<path d="m30 6 8 8-8 8" />
				</>
			)}
			{kind === "rectangle" && <rect x="7" y="5" width="34" height="18" rx="1.5" />}
			{kind === "ellipse" && <ellipse cx="24" cy="14" rx="18" ry="9" />}
			{kind === "circle" && <circle cx="24" cy="14" r="10" />}
			{kind === "line" && <path d="M7 21 41 7" />}
			{kind === "highlight" && (
				<path d="M7 14h34" strokeWidth="9" strokeOpacity="0.55" />
			)}
		</svg>
	);
}

export function AnnotationSettingsPanel({
	annotation,
	onContentChange,
	onTypeChange,
	onStyleChange,
	onFigureDataChange,
	onDrawingDataChange,
	onBlurIntensityChange,
	onBlurColorChange,
	onClickDataChange,
	onCalloutEventChange,
	onStagesChange,
}: AnnotationSettingsPanelProps) {
	const t = useScopedT("editor");
	const fileInputRef = useRef<HTMLInputElement>(null);
	const [customFonts, setCustomFonts] = useState<CustomFont[]>([]);
	const stages = { ...DEFAULT_ANNOTATION_STAGES, ...annotation.stages };
	const stageDurationLimit = Math.max(0, annotation.endMs - annotation.startMs);
	const figureKind = annotation.figureData?.kind ?? "arrow";
	const figureSupportsFill = figureKind !== "arrow" && figureKind !== "line";
	const figureFillEnabled = annotation.figureData?.fillEnabled ?? figureKind === "highlight";
	const figureOutlineEnabled = annotation.figureData?.outlineEnabled ?? figureKind !== "highlight";
	const figureFillGradient = annotation.figureData?.fillGradient ?? (figureKind === "highlight" ? annotation.figureData?.gradient : undefined);

	const fontFamilies = useMemo(
		() => FONT_FAMILY_VALUES.map((f) => ({ value: f.value, label: t(f.labelKey) })),
		[t],
	);

	// Load custom fonts on mount
	useEffect(() => {
		setCustomFonts(getCustomFonts());
	}, []);

	const colorPalette = [
		"#FF0000", // Red
		"#FFD700", // Yellow/Gold
		"#00FF00", // Green
		"#FFFFFF", // White
		"#0000FF", // Blue
		"#FF6B00", // Orange
		"#9B59B6", // Purple
		"#E91E63", // Pink
		"#00BCD4", // Cyan
		"#FF5722", // Deep Orange
		"#8BC34A", // Light Green
		"#FFC107", // Amber
		"#2563EB", // Brand Blue
		"#000000", // Black
		"#607D8B", // Blue Grey
		"#795548", // Brown
	];

	const handleImageUpload = (event: React.ChangeEvent<HTMLInputElement>) => {
		const files = event.target.files;
		if (!files || files.length === 0) return;

		const file = files[0];

		// Validate file type
		const validTypes = ["image/jpeg", "image/jpg", "image/png", "image/gif", "image/webp"];
		if (!validTypes.includes(file.type)) {
			toast.error(t("annotations.imageUploadError"), {
				description: t("annotations.imageUploadErrorDescription"),
			});
			event.target.value = "";
			return;
		}

		const reader = new FileReader();

		reader.onload = (e) => {
			const dataUrl = e.target?.result as string;
			if (dataUrl) {
				onContentChange(dataUrl);
				toast.success(t("annotations.imageUploadSuccess"));
			}
		};

		reader.onerror = () => {
			toast.error(t("annotations.imageUploadFailed"), {
				description: t("annotations.imageUploadFailedDescription"),
			});
		};

		reader.readAsDataURL(file);
		event.target.value = "";
	};

	return (
		<Card className="flex min-h-0 flex-1 flex-col gap-0 overflow-hidden rounded-none bg-transparent p-0 shadow-none">
			<div className="flex-1 min-h-0 px-5 pb-6 pt-1 overflow-y-auto custom-scrollbar">
				<div className="mb-6">
					{/* Type Selector */}
					<div className="space-y-4">
						<ChoiceGroup
							aria-label="Annotation type"
							value={annotation.type}
							onValueChange={(value) => onTypeChange(value as AnnotationType)}
							className="grid grid-cols-7 gap-2"
						>
							<ChoiceItem
								value="text"
								aria-label={t("annotations.text")}
								title={t("annotations.text")}
								className="aspect-square min-w-0 justify-center p-0"
							>
								<Type className="w-4 h-4" />
							</ChoiceItem>
							<ChoiceItem
								value="image"
								aria-label={t("annotations.image")}
								title={t("annotations.image")}
								className="aspect-square min-w-0 justify-center p-0"
							>
								<ImageIcon className="w-4 h-4" />
							</ChoiceItem>
							<ChoiceItem
								value="figure"
								aria-label={t("annotations.arrow")}
								title={t("annotations.arrow")}
								className="aspect-square min-w-0 justify-center p-0"
							>
								<svg
									className="w-4 h-4"
									viewBox="0 0 24 24"
									fill="none"
									stroke="currentColor"
									strokeWidth="2"
								>
									<path
										d="M4 12h16m0 0l-6-6m6 6l-6 6"
										strokeLinecap="round"
										strokeLinejoin="round"
									/>
								</svg>
							</ChoiceItem>
							<ChoiceItem
								value="blur"
								aria-label={t("annotations.blur")}
								title={t("annotations.blur")}
								className="aspect-square min-w-0 justify-center p-0"
							>
								<SquareDashed className="w-4 h-4" />
							</ChoiceItem>
							<ChoiceItem
								value="drawing"
								aria-label={t("annotations.draw", "Draw")}
								title={t("annotations.draw", "Draw")}
								className="aspect-square min-w-0 justify-center p-0"
							>
								<Pencil className="w-4 h-4" />
							</ChoiceItem>
							<ChoiceItem
								value="click"
								aria-label={t("annotations.clickHighlight", "Click highlight")}
								title={t("annotations.clickHighlight", "Click highlight")}
								className="aspect-square min-w-0 justify-center p-0"
							>
								<Circle className="w-4 h-4" />
							</ChoiceItem>
							<ChoiceItem value="cursor-callout" aria-label={t("annotations.cursorCallout", "Cursor label event")} title={t("annotations.cursorCallout", "Cursor label event")} className="aspect-square min-w-0 justify-center p-0">
								<ChatText className="w-4 h-4" />
							</ChoiceItem>
						</ChoiceGroup>

						{/* A timeline annotation has three visual phases. Durations move its two
						 * internal boundaries; disabled phases are rendered as Wait. */}
						<div className="rounded-lg border border-foreground/10 bg-foreground/[0.025] p-3 space-y-3">
							<div className="flex items-center justify-between">
								<div>
									<div className="text-sm font-medium">
										{t("annotations.animationStages", "Animation stages")}
									</div>
									<div className="text-xs text-muted-foreground">
										{t("annotations.stageFlow", "In → Wait → Out")}
									</div>
								</div>
								<div className="text-xs text-muted-foreground">
									{Math.round(stageDurationLimit)} ms
								</div>
							</div>
							{(["in", "out"] as const).map((phase) => {
								const enabledKey = phase === "in" ? "inEnabled" : "outEnabled";
								const durationKey =
									phase === "in" ? "inDurationMs" : "outDurationMs";
								const transitionKey =
									phase === "in" ? "inTransition" : "outTransition";
								const otherDuration =
									phase === "in" ? stages.outDurationMs : stages.inDurationMs;
								const enabled = stages[enabledKey];
								return (
									<div
										key={phase}
										className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-2 items-center"
									>
										<ToggleButton
											isSelected={enabled}
											onClick={() =>
												onStagesChange?.({ [enabledKey]: !enabled })
											}
											className="h-8 px-2 text-xs"
										>
											{phase === "in"
												? t("annotations.stageIn", "In")
												: t("annotations.stageOut", "Out")}
										</ToggleButton>
										<Select
											value={stages[transitionKey]}
											onValueChange={(value) =>
												onStagesChange?.({
													[transitionKey]: value as AnnotationTransition,
												})
											}
										>
											<SelectTrigger className="h-8 text-xs">
												<SelectValue />
											</SelectTrigger>
											<SelectContent>
												{(
													[
														"none",
														"fade",
														"pop",
														"slide-up",
														"slide-down",
														"slide-left",
														"slide-right",
														...(annotation.type === "text"
															? [
																	"bounce",
																	"zoom",
																	"swing",
																	"typewriter",
																]
															: []),
														...(annotation.type === "drawing"
															? [
																	phase === "in"
																		? "draw-in"
																		: "draw-out",
																]
															: []),
													] as AnnotationTransition[]
												).map((effect) => (
													<SelectItem key={effect} value={effect}>
														{effect === "draw-in"
															? t(
																	"annotations.animationDrawIn",
																	"Draw In",
																)
															: effect === "draw-out"
																? t(
																		"annotations.animationDrawOut",
																		"Draw Out",
																	)
																: effect === "typewriter"
																	? t(
																			"annotations.animationTypewriter",
																			"Typewriter",
																		)
																	: effect}
													</SelectItem>
												))}
											</SelectContent>
										</Select>
										<div className="col-span-2 flex items-center gap-3">
											<Slider
												aria-label={`${phase} duration`}
												value={[stages[durationKey]]}
												onValueChange={([value]) =>
													onStagesChange?.({
														[durationKey]: Math.min(
															value,
															Math.max(
																0,
																stageDurationLimit - otherDuration,
															),
														),
													})
												}
												min={0}
												max={Math.max(
													0,
													stageDurationLimit - otherDuration,
												)}
												step={10}
												className="flex-1"
											/>
											<span className="w-14 text-right text-xs tabular-nums text-muted-foreground">
												{Math.round(stages[durationKey])} ms
											</span>
										</div>
									</div>
								);
							})}
						</div>

						{/* Text Content */}
						{annotation.type === "cursor-callout" && (
							<div className="space-y-3">
								<div className="text-sm font-medium">{t("annotations.cursorCallout", "Cursor label event")}</div>
								<ChoiceGroup aria-label={t("annotations.cursorCalloutAction", "Action")} value={annotation.calloutEvent?.action ?? "enable"} onValueChange={(action) => onCalloutEventChange?.({ ...annotation.calloutEvent, action: action as "enable" | "disable" | "text" })} className="grid grid-cols-3 gap-2">
									<ChoiceItem value="enable">{t("annotations.calloutEnable", "Enable")}</ChoiceItem>
									<ChoiceItem value="text">{t("annotations.calloutText", "Text")}</ChoiceItem>
									<ChoiceItem value="disable">{t("annotations.calloutDisable", "Disable")}</ChoiceItem>
								</ChoiceGroup>
								{(annotation.calloutEvent?.action ?? "enable") === "text" && <TextArea value={annotation.content} onChange={(event) => onContentChange(event.target.value.slice(0, 80))} placeholder={t("annotations.calloutTextPlaceholder", "Label text")} />}
								<div className="space-y-3 rounded-lg border border-foreground/10 p-3">
									<div className="text-xs font-medium">{t("annotations.calloutStyle", "Label style")}</div>
									<div className="grid grid-cols-2 gap-2">
										<ColorControl value={annotation.calloutEvent?.style?.background ?? "#14B8C4"} label={t("annotations.calloutBackground", "Background")} colors={colorPalette} compact onChange={(background) => onCalloutEventChange?.({ action: annotation.calloutEvent?.action ?? "enable", style: { ...annotation.calloutEvent?.style, background } })} />
										<ColorControl value={annotation.calloutEvent?.style?.textColor ?? "#FFFFFF"} label={t("annotations.calloutTextColor", "Text color")} colors={colorPalette} compact onChange={(textColor) => onCalloutEventChange?.({ action: annotation.calloutEvent?.action ?? "enable", style: { ...annotation.calloutEvent?.style, textColor } })} />
									</div>
									<label className="block text-xs text-muted-foreground">{t("annotations.calloutScale", "Label size")}: {Math.round((annotation.calloutEvent?.style?.scale ?? 1) * 100)}%</label>
									<Slider value={[annotation.calloutEvent?.style?.scale ?? 1]} min={0.5} max={2} step={0.05} onValueChange={([scale]) => onCalloutEventChange?.({ action: annotation.calloutEvent?.action ?? "enable", style: { ...annotation.calloutEvent?.style, scale } })} />
									<ToggleButton isSelected={Boolean(annotation.calloutEvent?.style?.gradientEnabled)} onClick={() => onCalloutEventChange?.({ action: annotation.calloutEvent?.action ?? "enable", style: { ...annotation.calloutEvent?.style, gradientEnabled: !annotation.calloutEvent?.style?.gradientEnabled, gradientColors: annotation.calloutEvent?.style?.gradientColors ?? [...DEFAULT_CURSOR_CALLOUT_GRADIENT.colors], gradientGlow: annotation.calloutEvent?.style?.gradientGlow ?? DEFAULT_CURSOR_CALLOUT_GRADIENT.glow } })} className="h-9 w-full text-xs">
										{t("annotations.animatedGradient", "Animated gradient")}
									</ToggleButton>
									{annotation.calloutEvent?.style?.gradientEnabled && (
										<>
											<label className="block text-xs text-muted-foreground">{t("annotations.gradientColorCount", "Gradient colors")}</label>
											<Slider value={[annotation.calloutEvent.style.gradientColors?.length ?? 2]} min={2} max={4} step={1} onValueChange={([count]) => {
												const colors = [...(annotation.calloutEvent?.style?.gradientColors ?? DEFAULT_CURSOR_CALLOUT_GRADIENT.colors)];
												while (colors.length < count) colors.push(colors[colors.length - 1] ?? "#A855F7");
												onCalloutEventChange?.({ action: annotation.calloutEvent?.action ?? "enable", style: { ...annotation.calloutEvent?.style, gradientColors: colors.slice(0, count) } });
											}} />
											<div className="grid grid-cols-2 gap-2">
												{(annotation.calloutEvent.style.gradientColors ?? DEFAULT_CURSOR_CALLOUT_GRADIENT.colors).map((color, index) => <ColorControl key={index} value={color} label={`${t("annotations.gradientColor", "Gradient color")} ${index + 1}`} colors={colorPalette} compact onChange={(nextColor) => {
													const colors = [...(annotation.calloutEvent?.style?.gradientColors ?? DEFAULT_CURSOR_CALLOUT_GRADIENT.colors)];
													colors[index] = nextColor;
													onCalloutEventChange?.({ action: annotation.calloutEvent?.action ?? "enable", style: { ...annotation.calloutEvent?.style, gradientColors: colors } });
												}} />)}
											</div>
											<label className="block text-xs text-muted-foreground">{t("annotations.gradientGlow", "Glow strength")}</label>
											<Slider value={[annotation.calloutEvent.style.gradientGlow ?? DEFAULT_CURSOR_CALLOUT_GRADIENT.glow]} min={0} max={40} step={1} onValueChange={([gradientGlow]) => onCalloutEventChange?.({ action: annotation.calloutEvent?.action ?? "enable", style: { ...annotation.calloutEvent?.style, gradientGlow } })} />
										</>
									)}
								</div>
								<p className="text-xs text-muted-foreground">{t("annotations.calloutEventHint", "The event duration controls the typewriter animation.")}</p>
							</div>
						)}
						{annotation.type === "text" && (
							<div className="space-y-4">
								<div>
									<label className="text-sm font-medium text-foreground mb-2 block">
										{t("annotations.textContent")}
									</label>
									<TextArea
										value={annotation.textContent || annotation.content}
										onChange={(e) => onContentChange(e.target.value)}
										placeholder={t("annotations.textPlaceholder")}
										rows={3}
										className="w-full px-3 py-2 text-sm resize-none"
									/>
								</div>

								{/* Styling Controls */}
								<div className="space-y-4">
									{/* Font Family & Size */}
									<div className="grid grid-cols-2 gap-2">
										<div>
											<label className="text-sm font-medium text-foreground mb-2 block">
												{t("annotations.fontStyle")}
											</label>
											<Select
												value={annotation.style.fontFamily}
												onValueChange={(value) =>
													onStyleChange({ fontFamily: value })
												}
											>
												<SelectTrigger className="w-full h-9 text-xs">
													<SelectValue
														placeholder={t("annotations.selectStyle")}
													/>
												</SelectTrigger>
												<SelectContent className="max-h-[300px]">
													{!fontFamilies.some(
														(font) =>
															font.value ===
															annotation.style.fontFamily,
													) &&
														!customFonts.some(
															(font) =>
																font.fontFamily ===
																annotation.style.fontFamily,
														) && (
															<SelectItem
																value={annotation.style.fontFamily}
															>
																{annotation.style.fontFamily
																	.split(",")[0]
																	.replace(/"/g, "")}
															</SelectItem>
														)}
													{fontFamilies.map((font) => (
														<SelectItem
															key={font.value}
															value={font.value}
															style={{ fontFamily: font.value }}
														>
															{font.label}
														</SelectItem>
													))}
													{customFonts.length > 0 && (
														<>
															<div className="px-2 py-1.5 text-[10px] font-medium text-muted-foreground uppercase tracking-wider">
																Custom Fonts
															</div>
															{customFonts.map((font) => (
																<SelectItem
																	key={font.id}
																	value={font.fontFamily}
																	style={{
																		fontFamily: font.fontFamily,
																	}}
																>
																	{font.name}
																</SelectItem>
															))}
														</>
													)}
												</SelectContent>
											</Select>
										</div>
										<div>
											<label className="text-sm font-medium text-foreground mb-2 block">
												{t("annotations.size")}
											</label>
											<Select
												value={annotation.style.fontSize.toString()}
												onValueChange={(value) =>
													onStyleChange({ fontSize: parseInt(value) })
												}
											>
												<SelectTrigger className="w-full h-9 text-xs">
													<SelectValue
														placeholder={t("annotations.size")}
													/>
												</SelectTrigger>
												<SelectContent className="max-h-[200px]">
													{FONT_SIZES.map((size) => (
														<SelectItem
															key={size}
															value={size.toString()}
														>
															{size}px
														</SelectItem>
													))}
												</SelectContent>
											</Select>
										</div>
									</div>

									{/* Add Custom Font Button */}
									<div>
										<AddCustomFontDialog
											onFontAdded={(font) => {
												setCustomFonts(getCustomFonts());
												onStyleChange({ fontFamily: font.fontFamily });
											}}
										/>
									</div>

									{/* Formatting Toggles */}
									<div className="flex flex-wrap items-center justify-between gap-2">
										<ToggleGroup
											type="multiple"
											value={[
												...(annotation.style.fontWeight === "bold"
													? ["bold"]
													: []),
												...(annotation.style.fontStyle === "italic"
													? ["italic"]
													: []),
												...(annotation.style.textDecoration === "underline"
													? ["underline"]
													: []),
											]}
											size="sm"
											className="justify-start"
										>
											<ToggleGroupItem
												value="bold"
												aria-label={t("annotations.toggleBold")}
												onClick={() =>
													onStyleChange({
														fontWeight:
															annotation.style.fontWeight === "bold"
																? "normal"
																: "bold",
													})
												}
												className="h-8 w-8 min-w-8 p-0"
											>
												<Bold className="h-4 w-4" />
											</ToggleGroupItem>
											<ToggleGroupItem
												value="italic"
												aria-label={t("annotations.toggleItalic")}
												onClick={() =>
													onStyleChange({
														fontStyle:
															annotation.style.fontStyle === "italic"
																? "normal"
																: "italic",
													})
												}
												className="h-8 w-8 min-w-8 p-0"
											>
												<Italic className="h-4 w-4" />
											</ToggleGroupItem>
											<ToggleGroupItem
												value="underline"
												aria-label={t("annotations.toggleUnderline")}
												onClick={() =>
													onStyleChange({
														textDecoration:
															annotation.style.textDecoration ===
															"underline"
																? "none"
																: "underline",
													})
												}
												className="h-8 w-8 min-w-8 p-0"
											>
												<Underline className="h-4 w-4" />
											</ToggleGroupItem>
										</ToggleGroup>

										<ToggleGroup
											type="single"
											value={annotation.style.textAlign}
											size="sm"
											className="justify-start"
										>
											<ToggleGroupItem
												value="left"
												aria-label={t("annotations.alignLeft")}
												onClick={() => onStyleChange({ textAlign: "left" })}
												className="h-8 w-8 min-w-8 p-0"
											>
												<AlignLeft className="h-4 w-4" />
											</ToggleGroupItem>
											<ToggleGroupItem
												value="center"
												aria-label={t("annotations.alignCenter")}
												onClick={() =>
													onStyleChange({ textAlign: "center" })
												}
												className="h-8 w-8 min-w-8 p-0"
											>
												<AlignCenter className="h-4 w-4" />
											</ToggleGroupItem>
											<ToggleGroupItem
												value="right"
												aria-label={t("annotations.alignRight")}
												onClick={() =>
													onStyleChange({ textAlign: "right" })
												}
												className="h-8 w-8 min-w-8 p-0"
											>
												<AlignRight className="h-4 w-4" />
											</ToggleGroupItem>
										</ToggleGroup>
									</div>

									{/* Colors */}
									<div className="grid grid-cols-2 gap-4">
										<div>
											<label className="text-sm font-medium text-foreground mb-2 block">
												{t("annotations.textColor")}
											</label>
											<ColorControl
												value={annotation.style.color}
												label={t("annotations.textColor")}
												onChange={(color) => onStyleChange({ color })}
												colors={colorPalette}
												compact
											/>
										</div>
										<div>
											<label className="text-sm font-medium text-foreground mb-2 block">
												{t("annotations.background")}
											</label>
											<ColorControl
												value={annotation.style.backgroundColor}
												label={t("annotations.background")}
												onChange={(color) =>
													onStyleChange({ backgroundColor: color })
												}
												colors={colorPalette}
												compact
												onClear={() =>
													onStyleChange({
														backgroundColor: "transparent",
													})
												}
												clearLabel={t(
													"annotations.transparent",
													"Transparent",
												)}
											/>
										</div>
									</div>
								</div>
							</div>
						)}

						{/* Image Upload */}
						{annotation.type === "image" && (
							<div className="space-y-4">
								<input
									type="file"
									ref={fileInputRef}
									onChange={handleImageUpload}
									accept=".jpg,.jpeg,.png,.gif,.webp,image/*"
									className="hidden"
								/>
								<Button
									onClick={() => fileInputRef.current?.click()}
									variant="outline"
									className="w-full gap-2 py-8"
								>
									<Upload className="w-5 h-5" />
									{t("annotations.uploadImage")}
								</Button>

								{annotation.content &&
									annotation.content.startsWith("data:image") && (
										<div className="rounded-lg border border-foreground/10 overflow-hidden bg-foreground/5 p-2">
											<img
												src={annotation.content}
												alt="Uploaded annotation"
												className="w-full h-auto rounded-md"
											/>
										</div>
									)}

								<p className="text-xs text-muted-foreground/70 text-center leading-relaxed">
									{t("annotations.supportedFormats")}
								</p>
							</div>
						)}

						{annotation.type === "figure" && (
							<div className="space-y-4">
								<div>
									<label className="text-sm font-medium text-foreground mb-3 block">
										{t("annotations.shape", "Shape")}
									</label>
									<ChoiceGroup
										value={annotation.figureData?.kind ?? "arrow"}
										onValueChange={(kind) =>
											onFigureDataChange?.({
												...annotation.figureData!,
												kind: kind as NonNullable<FigureData["kind"]>,
											})
										}
										className="grid grid-cols-3 gap-2"
										aria-label={t("annotations.shape", "Shape")}
									>
									{(
										["arrow", "rectangle", "ellipse", "circle", "line", "highlight"] as FigureKind[]
									).map((kind) => {
										const label =
											kind === "rectangle"
												? t("annotations.rectangle", "Rectangle")
												: kind === "ellipse"
													? t("annotations.ellipse", "Ellipse")
													: kind === "circle"
														? t("annotations.circle", "Circle")
														: kind === "highlight"
															? t("annotations.highlight", "Highlight")
															: kind === "line"
																? t("annotations.line", "Line")
																: t("annotations.arrow", "Arrow");
										return (
											<ChoiceItem
												key={kind}
												value={kind}
												aria-label={label}
												title={label}
												className="h-12 min-w-0 px-1"
											>
												<FigureKindPreview kind={kind} />
												<span className="sr-only">{label}</span>
											</ChoiceItem>
										);
									})}
									</ChoiceGroup>
								</div>
								<div>
									<label className="mb-2 block text-sm font-medium">{t("annotations.figureOpacity", "Opacity")}: {Math.round((annotation.figureData?.opacity ?? (figureKind === "highlight" ? 0.28 : 1)) * 100)}%</label>
									<Slider aria-label={t("annotations.figureOpacity", "Opacity")} value={[annotation.figureData?.opacity ?? (figureKind === "highlight" ? 0.28 : 1)]} min={0} max={1} step={0.01} onValueChange={([opacity]) => onFigureDataChange?.({ ...annotation.figureData!, opacity })} />
								</div>
								{figureSupportsFill && (
									<div className="space-y-3 rounded-lg border border-foreground/10 p-3">
										<ToggleButton isSelected={figureFillEnabled} onClick={() => onFigureDataChange?.({ ...annotation.figureData!, fillEnabled: !figureFillEnabled })} className="h-9 w-full text-xs">{t("annotations.figureFill", "Fill")}</ToggleButton>
										{figureFillEnabled && <>
											<ColorControl value={annotation.figureData?.fillColor ?? annotation.figureData?.color ?? "#2563EB"} label={t("annotations.fillColor", "Fill color")} colors={colorPalette} compact onChange={(fillColor) => onFigureDataChange?.({ ...annotation.figureData!, fillColor })} />
											<ChoiceGroup aria-label={t("annotations.fillMode", "Fill mode")} value={figureFillGradient?.enabled ? "gradient" : "solid"} onValueChange={(mode) => onFigureDataChange?.({ ...annotation.figureData!, fillEnabled: true, fillGradient: { ...DEFAULT_ANNOTATION_GRADIENT, ...figureFillGradient, enabled: mode === "gradient" } })} className="grid grid-cols-2 gap-2">
												<ChoiceItem value="solid">{t("annotations.solidFill", "Solid")}</ChoiceItem>
												<ChoiceItem value="gradient">{t("annotations.gradientFill", "Animated gradient")}</ChoiceItem>
											</ChoiceGroup>
											{figureFillGradient?.enabled && <>
												<label className="block text-xs text-muted-foreground">{t("annotations.gradientColorCount", "Gradient colors")}</label>
												<Slider value={[Math.min(4, Math.max(2, figureFillGradient.colors.length))]} min={2} max={4} step={1} onValueChange={([count]) => {
													const colors = [...figureFillGradient.colors];
													while (colors.length < count) colors.push(colors[colors.length - 1] ?? "#A855F7");
													onFigureDataChange?.({ ...annotation.figureData!, fillGradient: { ...DEFAULT_ANNOTATION_GRADIENT, ...figureFillGradient, colors: colors.slice(0, count) } });
												}} />
												<div className="grid grid-cols-2 gap-2">{figureFillGradient.colors.slice(0, 4).map((color, index) => <ColorControl key={index} value={color} label={`${t("annotations.gradientColor", "Gradient color")} ${index + 1}`} colors={colorPalette} compact onChange={(nextColor) => {
													const colors = [...figureFillGradient.colors];
													colors[index] = nextColor;
												onFigureDataChange?.({ ...annotation.figureData!, fillGradient: { ...DEFAULT_ANNOTATION_GRADIENT, ...figureFillGradient, colors } });
												}} />)}</div>
												<label className="block text-xs text-muted-foreground">{t("annotations.gradientGlow", "Glow strength")}</label>
												<Slider value={[figureFillGradient.glow]} min={0} max={40} step={1} onValueChange={([glow]) => onFigureDataChange?.({ ...annotation.figureData!, fillGradient: { ...DEFAULT_ANNOTATION_GRADIENT, ...figureFillGradient, glow } })} />
											</>}
										</>}
									</div>
								)}
								{figureSupportsFill && <ToggleButton isSelected={figureOutlineEnabled} onClick={() => onFigureDataChange?.({ ...annotation.figureData!, outlineEnabled: !figureOutlineEnabled })} className="h-9 w-full text-xs">{t("annotations.figureOutline", "Outline")}</ToggleButton>}
								{(annotation.figureData?.kind ?? "arrow") === "arrow" && (
									<div>
									<label className="text-sm font-medium text-foreground mb-3 block">
										{t("annotations.arrowDirection")}
									</label>
									<div className="grid grid-cols-4 gap-2">
										{(
											[
												"up",
												"down",
												"left",
												"right",
												"up-right",
												"up-left",
												"down-right",
												"down-left",
											] as ArrowDirection[]
										).map((direction) => {
											const ArrowComponent = getArrowComponent(direction);
											return (
												<ToggleButton
													isSelected={
														annotation.figureData?.arrowDirection ===
														direction
													}
													key={direction}
													onClick={() => {
														const newFigureData: FigureData = {
															...annotation.figureData!,
															arrowDirection: direction,
														};
														onFigureDataChange?.(newFigureData);
													}}
													aria-label={t(
														"annotations.arrowDirectionOption",
														"Arrow direction: {{direction}}",
														{ direction: direction.replace(/-/g, " ") },
													)}
													className={cn(
														"h-16 flex items-center justify-center p-2",
													)}
												>
													<ArrowComponent
														color={
															annotation.figureData
																?.arrowDirection === direction
																? "#ffffff"
																: "#94a3b8"
														}
														strokeWidth={3}
													/>
												</ToggleButton>
											);
										})}
									</div>
									</div>
								)}

								{(!figureSupportsFill || figureOutlineEnabled) && <>
								<div>
									<label className="text-sm font-medium text-foreground mb-2 block">
										{t("annotations.strokeWidth", undefined, {
											width: annotation.figureData?.strokeWidth || 4,
										})}
									</label>
									<Slider
										aria-label={t("annotations.strokeWidth", undefined, {
											width: annotation.figureData?.strokeWidth || 4,
										})}
										value={[annotation.figureData?.strokeWidth || 4]}
										onValueChange={([value]) => {
											const newFigureData: FigureData = {
												...annotation.figureData!,
												strokeWidth: value,
											};
											onFigureDataChange?.(newFigureData);
										}}
										min={1}
										max={6}
										step={1}
										className="my-3 w-full"
									/>
								</div>

								<div>
									<label className="text-sm font-medium text-foreground mb-2 block">
										{t("annotations.outlineColor", "Outline color")}
									</label>
									<ColorControl
										value={annotation.figureData?.color || "#2563EB"}
										label={t("annotations.outlineColor", "Outline color")}
										onChange={(color) =>
											onFigureDataChange?.({
												...annotation.figureData!,
												color,
											})
										}
										colors={colorPalette}
										compact
									/>
								</div>
								<div className="space-y-3 rounded-lg border border-foreground/10 p-3">
									<ToggleButton
										isSelected={Boolean(
											annotation.figureData?.gradient?.enabled,
										)}
										onClick={() =>
											onFigureDataChange?.({
												...annotation.figureData!,
												gradient: {
													...DEFAULT_ANNOTATION_GRADIENT,
													...annotation.figureData?.gradient,
													enabled:
														!annotation.figureData?.gradient?.enabled,
												},
											})
										}
										className="h-9 w-full text-xs"
									>
										{t("annotations.outlineGradient", "Animated outline gradient")}
									</ToggleButton>
									{annotation.figureData?.gradient?.enabled && (
										<>
											<div>
												<label className="mb-1 block text-xs text-muted-foreground">{t("annotations.gradientColorCount", "Gradient colors")}</label>
												<Slider value={[Math.min(4, Math.max(2, annotation.figureData.gradient.colors.length))]} min={2} max={4} step={1} onValueChange={([count]) => {
													const colors = [...annotation.figureData!.gradient!.colors];
													while (colors.length < count) colors.push(colors[colors.length - 1] ?? "#A855F7");
													onFigureDataChange?.({ ...annotation.figureData!, gradient: { ...DEFAULT_ANNOTATION_GRADIENT, ...annotation.figureData!.gradient!, colors: colors.slice(0, count) } });
												}} />
											</div>
											<div className="grid grid-cols-2 gap-2">
											{(
												annotation.figureData.gradient.colors ??
												DEFAULT_ANNOTATION_GRADIENT.colors
											)
												.slice(0, 4)
												.map((color, index) => (
													<ColorControl
														key={index}
														value={color}
														label={`${t("annotations.gradientColor", "Gradient color")} ${index + 1}`}
														compact
														onChange={(nextColor) => {
															const colors = [
																...(annotation.figureData?.gradient
																	?.colors ??
																	DEFAULT_ANNOTATION_GRADIENT.colors),
															];
															colors[index] = nextColor;
															onFigureDataChange?.({
																...annotation.figureData!,
																gradient: {
																	...DEFAULT_ANNOTATION_GRADIENT,
																	...annotation.figureData
																		?.gradient,
																	colors,
																},
															});
														}}
													/>
												))}
											</div>
											<div>
												<label className="mb-1 block text-xs text-muted-foreground">{t("annotations.gradientGlow", "Glow strength")}</label>
												<Slider value={[annotation.figureData.gradient.glow ?? DEFAULT_ANNOTATION_GRADIENT.glow]} min={0} max={40} step={1} onValueChange={([glow]) => onFigureDataChange?.({ ...annotation.figureData!, gradient: { ...DEFAULT_ANNOTATION_GRADIENT, ...annotation.figureData!.gradient!, glow } })} />
											</div>
										</>
									)}
								</div>
								</>}
							</div>
						)}

						{annotation.type === "blur" && (
							<div className="space-y-4">
								<div className="flex flex-col items-center">
									<div className="w-full space-y-5">
										<div className="flex items-center justify-between">
											<span className="text-sm font-medium text-foreground">
												{t("annotations.blurStrength", undefined, {
													strength: annotation.blurIntensity ?? 20,
												})}
											</span>
										</div>
										<Slider
											aria-label={t("annotations.blurStrength", undefined, {
												strength: annotation.blurIntensity ?? 20,
											})}
											value={[annotation.blurIntensity ?? 20]}
											onValueChange={([value]) =>
												onBlurIntensityChange?.(value)
											}
											min={1}
											max={100}
											step={1}
											className="w-full"
										/>
									</div>

									<div className="w-full space-y-5 mt-4">
										<div className="flex items-center justify-between">
											<span className="text-sm font-medium text-foreground">
												{t(
													"annotations.solidColor",
													"Solid Color (Censorship)",
												)}
											</span>
										</div>
										<div className="flex flex-col gap-3">
											<Button
												variant="secondary"
												aria-pressed={
													!annotation.blurColor ||
													annotation.blurColor === "transparent"
												}
												onClick={() => onBlurColorChange?.("")}
											>
												{t("annotations.none", "None")}
											</Button>
											<ColorPalette
												color={annotation.blurColor || "transparent"}
												colors={colorPalette}
												onChange={({ hex }) => onBlurColorChange?.(hex)}
											/>
											<ColorControl
												label={t("annotations.customColor", "Custom color")}
												value={annotation.blurColor || "#000000"}
												onChange={(color) => onBlurColorChange?.(color)}
											/>
										</div>
									</div>
								</div>
							</div>
						)}

						{annotation.type === "drawing" && (
							<div className="space-y-4">
								<p className="rounded-md bg-foreground/5 p-3 text-xs text-muted-foreground">
									{t(
										"annotations.drawHint",
										"Draw with the left mouse button. Hold Shift for a straight line. Right-drag moves the drawing area.",
									)}
								</p>
								<div className="grid grid-cols-3 gap-2">
									<ToggleButton
										isSelected={
											(annotation.drawingData?.tool ?? "pencil") === "pencil"
										}
										onClick={() =>
											onDrawingDataChange?.({
												...annotation.drawingData!,
												points: annotation.drawingData?.points ?? [],
												color: annotation.drawingData?.color ?? "#2563EB",
												strokeWidth:
													annotation.drawingData?.strokeWidth ?? 4,
												tool: "pencil",
											})
										}
										className="h-9 gap-1 text-xs"
									>
										<Pencil className="h-4 w-4" />{" "}
										{t("annotations.pencil", "Pencil")}
									</ToggleButton>
									<ToggleButton
										isSelected={annotation.drawingData?.tool === "eraser"}
										onClick={() =>
											onDrawingDataChange?.({
												...annotation.drawingData!,
												points: annotation.drawingData?.points ?? [],
												color: annotation.drawingData?.color ?? "#2563EB",
												strokeWidth:
													annotation.drawingData?.strokeWidth ?? 4,
												tool: "eraser",
											})
										}
										className="h-9 gap-1 text-xs"
									>
										<Eraser className="h-4 w-4" />{" "}
										{t("annotations.eraser", "Eraser")}
									</ToggleButton>
									<Button
										variant="secondary"
										size="sm"
										onClick={() =>
											onDrawingDataChange?.({
												...annotation.drawingData!,
												points: [],
												strokes: [],
												color: annotation.drawingData?.color ?? "#2563EB",
												strokeWidth:
													annotation.drawingData?.strokeWidth ?? 4,
											})
										}
										title={t("annotations.clearDrawing", "Clear drawing")}
									>
										<Trash2 className="h-4 w-4" />
									</Button>
								</div>
								{annotation.stages?.outEnabled &&
									annotation.stages.outTransition === "draw-out" && (
										<ToggleButton
											isSelected={Boolean(
												annotation.drawingData?.showEraserInDrawOut,
											)}
											onClick={() =>
												onDrawingDataChange?.({
													...annotation.drawingData!,
													points: annotation.drawingData?.points ?? [],
													color: annotation.drawingData?.color ?? "#2563EB",
													strokeWidth: annotation.drawingData?.strokeWidth ?? 4,
													showEraserInDrawOut:
														!annotation.drawingData?.showEraserInDrawOut,
												})
											}
											className="h-9 w-full text-xs"
										>
											{t("annotations.showEraserInDrawOut", "Show eraser in Draw Out")}
										</ToggleButton>
									)}
								<div>
									<label className="text-sm font-medium text-foreground mb-2 block">
										{t("annotations.brushWidth", "Brush width")}
									</label>
									<Slider
										value={[annotation.drawingData?.strokeWidth ?? 4]}
										min={1}
										max={24}
										step={1}
										onValueChange={([strokeWidth]) =>
											onDrawingDataChange?.({
												points: annotation.drawingData?.points ?? [],
												strokes: annotation.drawingData?.strokes,
												color: annotation.drawingData?.color ?? "#2563EB",
												strokeWidth,
											})
										}
									/>
								</div>
								<div>
									<label className="text-sm font-medium text-foreground mb-2 block">
										{t("annotations.eraserSize", "Eraser size")}
									</label>
									<Slider
										value={[annotation.drawingData?.eraserSize ?? 12]}
										min={4}
										max={48}
										step={1}
										onValueChange={([eraserSize]) =>
											onDrawingDataChange?.({
												...annotation.drawingData!,
												points: annotation.drawingData?.points ?? [],
												color: annotation.drawingData?.color ?? "#2563EB",
												strokeWidth:
													annotation.drawingData?.strokeWidth ?? 4,
												eraserSize,
											})
										}
									/>
								</div>
								<ColorControl
									label={t("annotations.brushColor", "Brush color")}
									value={annotation.drawingData?.color ?? "#2563EB"}
									colors={colorPalette}
									compact
									onChange={(color) =>
										onDrawingDataChange?.({
											points: annotation.drawingData?.points ?? [],
											strokes: annotation.drawingData?.strokes,
											strokeWidth: annotation.drawingData?.strokeWidth ?? 4,
											color,
										})
									}
								/>
								<div className="space-y-3 rounded-lg border border-foreground/10 p-3">
									<ToggleButton
										isSelected={Boolean(
											annotation.drawingData?.gradient?.enabled,
										)}
										onClick={() =>
											onDrawingDataChange?.({
												...annotation.drawingData!,
												points: annotation.drawingData?.points ?? [],
												color: annotation.drawingData?.color ?? "#2563EB",
												strokeWidth:
													annotation.drawingData?.strokeWidth ?? 4,
												gradient: {
													...DEFAULT_ANNOTATION_GRADIENT,
													...annotation.drawingData?.gradient,
													enabled:
														!annotation.drawingData?.gradient?.enabled,
												},
											})
										}
										className="h-9 w-full text-xs"
									>
										{t("annotations.animatedGradient", "Animated gradient")}
									</ToggleButton>
									{annotation.drawingData?.gradient?.enabled && (
										<>
											<div>
												<label className="mb-1 block text-xs text-muted-foreground">{t("annotations.gradientColorCount", "Gradient colors")}</label>
												<Slider value={[Math.min(4, Math.max(2, annotation.drawingData.gradient.colors.length))]} min={2} max={4} step={1} onValueChange={([count]) => {
													const colors = [...annotation.drawingData!.gradient!.colors];
													while (colors.length < count) colors.push(colors[colors.length - 1] ?? "#A855F7");
													onDrawingDataChange?.({ ...annotation.drawingData!, points: annotation.drawingData?.points ?? [], color: annotation.drawingData?.color ?? "#2563EB", strokeWidth: annotation.drawingData?.strokeWidth ?? 4, gradient: { ...DEFAULT_ANNOTATION_GRADIENT, ...annotation.drawingData!.gradient!, colors: colors.slice(0, count) } });
												}} />
											</div>
											<div className="grid grid-cols-2 gap-2">
											{(
												annotation.drawingData.gradient.colors ??
												DEFAULT_ANNOTATION_GRADIENT.colors
											)
												.slice(0, 4)
												.map((color, index) => (
													<ColorControl
														key={index}
														value={color}
														label={`${t("annotations.gradientColor", "Gradient color")} ${index + 1}`}
														compact
														onChange={(nextColor) => {
															const colors = [
																...(annotation.drawingData?.gradient
																	?.colors ??
																	DEFAULT_ANNOTATION_GRADIENT.colors),
															];
															colors[index] = nextColor;
															onDrawingDataChange?.({
																...annotation.drawingData!,
																points:
																	annotation.drawingData
																		?.points ?? [],
																color:
																	annotation.drawingData?.color ??
																	"#2563EB",
																strokeWidth:
																	annotation.drawingData
																		?.strokeWidth ?? 4,
																gradient: {
																	...DEFAULT_ANNOTATION_GRADIENT,
																	...annotation.drawingData
																		?.gradient,
																	colors,
																},
															});
														}}
													/>
												))}
											</div>
											<div>
												<label className="mb-1 block text-xs text-muted-foreground">{t("annotations.gradientGlow", "Glow strength")}</label>
												<Slider value={[annotation.drawingData.gradient.glow ?? DEFAULT_ANNOTATION_GRADIENT.glow]} min={0} max={40} step={1} onValueChange={([glow]) => onDrawingDataChange?.({ ...annotation.drawingData!, points: annotation.drawingData?.points ?? [], color: annotation.drawingData?.color ?? "#2563EB", strokeWidth: annotation.drawingData?.strokeWidth ?? 4, gradient: { ...DEFAULT_ANNOTATION_GRADIENT, ...annotation.drawingData!.gradient!, glow } })} />
											</div>
										</>
									)}
								</div>
							</div>
						)}

						{annotation.type === "click" && (
							<div className="space-y-4">
								<p className="rounded-md bg-foreground/5 p-3 text-xs text-muted-foreground">
									{t(
										"annotations.clickHighlightHint",
										"A timed highlight: particles expand slightly, stop, then fade out.",
									)}
								</p>
								<ColorControl
									label={t("annotations.clickColor", "Circle color")}
									value={
										annotation.clickData?.color ??
										DEFAULT_CLICK_HIGHLIGHT_DATA.color
									}
									colors={colorPalette}
									compact
									onChange={(color) =>
										onClickDataChange?.({
											...DEFAULT_CLICK_HIGHLIGHT_DATA,
											...annotation.clickData,
											color,
										})
									}
								/>
								<div>
									<label className="text-sm font-medium">
										{t("annotations.particleCount", "Particles")}:{" "}
										{annotation.clickData?.particleCount ??
											DEFAULT_CLICK_HIGHLIGHT_DATA.particleCount}
									</label>
									<Slider
										value={[
											annotation.clickData?.particleCount ??
												DEFAULT_CLICK_HIGHLIGHT_DATA.particleCount,
										]}
										min={0}
										max={60}
										step={1}
										onValueChange={([particleCount]) =>
											onClickDataChange?.({
												...DEFAULT_CLICK_HIGHLIGHT_DATA,
												...annotation.clickData,
												particleCount,
											})
										}
									/>
								</div>
								<div>
									<label className="text-sm font-medium">
										{t("annotations.clickSize", "Size")}:{" "}
										{annotation.clickData?.radius ??
											DEFAULT_CLICK_HIGHLIGHT_DATA.radius}
										px
									</label>
									<Slider
										value={[
											annotation.clickData?.radius ??
												DEFAULT_CLICK_HIGHLIGHT_DATA.radius,
										]}
										min={20}
										max={300}
										step={1}
										onValueChange={([radius]) =>
											onClickDataChange?.({
												...DEFAULT_CLICK_HIGHLIGHT_DATA,
												...annotation.clickData,
												radius,
											})
										}
									/>
								</div>
							</div>
						)}
					</div>

					<details className="mt-4 text-muted-foreground">
						<summary className="flex cursor-pointer items-center gap-2 py-2">
							<Info className="w-3.5 h-3.5" />
							<span className="text-xs font-medium">
								{t("annotations.shortcutsAndTips")}
							</span>
						</summary>
						<ul className="text-xs text-muted-foreground space-y-1.5 list-disc pl-3 leading-relaxed">
							<li>{t("annotations.tipSelectAnnotation")}</li>
							<li>{t("annotations.tipCycleForward")}</li>
							<li>{t("annotations.tipCycleBackward")}</li>
						</ul>
					</details>
				</div>
			</div>
		</Card>
	);
}
