export function getTimelineEdgePanDelta(
	clientX: number,
	rect: DOMRect,
	sidebarWidth: number,
	visibleSpanMs: number,
	direction: string,
): number {
	const left = rect.left + (direction === "rtl" ? 0 : sidebarWidth);
	const right = rect.right - (direction === "rtl" ? sidebarWidth : 0);
	const edgeWidth = 32;
	const leftPressure = Math.max(0, Math.min(1, (left + edgeWidth - clientX) / edgeWidth));
	const rightPressure = Math.max(0, Math.min(1, (clientX - right + edgeWidth) / edgeWidth));
	const logicalDirection = direction === "rtl" ? -1 : 1;
	return (rightPressure - leftPressure) * logicalDirection * visibleSpanMs * 0.018;
}
