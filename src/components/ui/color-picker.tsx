import {
	ColorArea,
	ColorField,
	ColorPicker,
	ColorSlider,
	ColorSwatch,
	ColorSwatchPicker,
	Input,
	Label,
	Popover,
} from "@heroui/react";
import type { Color } from "@react-types/color";
import { Button } from "./button";

type PaletteProps = {
	color?: string;
	colors: readonly string[];
	onChange: (color: { hex: string }) => void;
	style?: React.CSSProperties;
};
export function ColorPalette({ color = "#000000", colors, onChange }: PaletteProps) {
	return (
		<div className="flex flex-col gap-3">
			<ColorSwatchPicker
				aria-label="Colors"
				value={color}
				onChange={(value) => onChange({ hex: value.toString("hex") })}
			>
				{colors.map((color) => (
					<ColorSwatchPicker.Item key={color} color={color}>
						<ColorSwatchPicker.Swatch className="ring-1 ring-inset ring-foreground/10" />
						<ColorSwatchPicker.Indicator />
					</ColorSwatchPicker.Item>
				))}
			</ColorSwatchPicker>
		</div>
	);
}
export function ColorControl({
	value,
	onChange,
	label,
	colors,
	compact = false,
	onClear,
	clearLabel = "Transparent",
}: {
	value: string;
	onChange: (value: string) => void;
	label: string;
	colors?: readonly string[];
	compact?: boolean;
	onClear?: () => void;
	clearLabel?: string;
}) {
	const activateHueFromNeutral = (color: Color) => {
		// A pure black/white value has zero saturation. Changing only its hue would
		// otherwise still serialize to the exact same grey and reset the picker.
		if (!/^#(?:000000|ffffff)$/i.test(value)) return;
		const hue = color.getChannelValue?.("hue") ?? Number.parseFloat(color.toString("hsb").match(/[-+]?[\d.]+/)?.[0] ?? "0");
		const normalizedHue = ((hue % 360) + 360) % 360;
		const chroma = 1 - Math.abs(((normalizedHue / 60) % 2) - 1);
		const [r, g, b] = normalizedHue < 60 ? [1, chroma, 0] : normalizedHue < 120 ? [chroma, 1, 0] : normalizedHue < 180 ? [0, 1, chroma] : normalizedHue < 240 ? [0, chroma, 1] : normalizedHue < 300 ? [chroma, 0, 1] : [1, 0, chroma];
		onChange(`#${[r, g, b].map((channel) => Math.round(channel * 255).toString(16).padStart(2, "0")).join("")}`);
	};
	return (
		<ColorPicker
			value={value === "transparent" ? "#00000000" : value}
			onChange={(color) => onChange(color.toString("hex"))}
		>
			<Button
				variant="secondary"
				aria-label={label}
				className="h-9 min-w-0 max-w-full gap-2 px-3 text-[13px]"
			>
				<ColorSwatch size="sm" />
				<span className="truncate">
					{compact ? (value === "transparent" ? "None" : value.toUpperCase()) : label}
				</span>
			</Button>
			<ColorPicker.Popover>
				<Popover.Dialog aria-label={label} className="flex w-64 flex-col gap-4 p-4">
					<ColorArea colorSpace="hsb" xChannel="saturation" yChannel="brightness">
						<ColorArea.Thumb />
					</ColorArea>
					<ColorSlider colorSpace="hsb" channel="hue" onChange={activateHueFromNeutral}>
						<ColorSlider.Track>
							<ColorSlider.Thumb />
						</ColorSlider.Track>
					</ColorSlider>
					{colors && (
						<ColorSwatchPicker aria-label="Preset colors">
							{colors.map((color) => (
								<ColorSwatchPicker.Item key={color} color={color}>
									<ColorSwatchPicker.Swatch />
									<ColorSwatchPicker.Indicator />
								</ColorSwatchPicker.Item>
							))}
						</ColorSwatchPicker>
					)}
					<ColorField>
						<Label>Hex color</Label>
						<Input />
					</ColorField>
					{onClear && (
						<Button variant="ghost" onClick={onClear}>
							{clearLabel}
						</Button>
					)}
				</Popover.Dialog>
			</ColorPicker.Popover>
		</ColorPicker>
	);
}
