import type { AnnotationRegion } from "./types";

/** Tracks are numbered from the video downwards; lower rows paint above higher ones. */
export function compareAnnotationLayerOrder(
	left: Pick<AnnotationRegion, "trackIndex" | "zIndex">,
	right: Pick<AnnotationRegion, "trackIndex" | "zIndex">,
): number {
	return (left.trackIndex ?? 0) - (right.trackIndex ?? 0) || left.zIndex - right.zIndex;
}
