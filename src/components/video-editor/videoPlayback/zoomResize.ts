import type { ZoomFocus } from "../types";

export type ZoomCorner = "nw" | "ne" | "sw" | "se";

/** Resize in normalized video space, keeping the opposite corner and aspect ratio. */
export function resizeZoomArea(anchor: ZoomFocus, pointer: ZoomFocus, corner: ZoomCorner) {
	const sx = corner.endsWith("e") ? 1 : -1;
	const sy = corner.startsWith("s") ? 1 : -1;
	const maxSize = Math.min(sx > 0 ? 1 - anchor.cx : anchor.cx, sy > 0 ? 1 - anchor.cy : anchor.cy);
	const size = Math.max(0.2, Math.min(maxSize, ((pointer.cx - anchor.cx) * sx + (pointer.cy - anchor.cy) * sy) / 2));
	return {
		scale: 1 / size,
		focus: { cx: anchor.cx + sx * size / 2, cy: anchor.cy + sy * size / 2 },
	};
}
