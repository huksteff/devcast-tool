import { type MouseEvent, type RefObject, useCallback, useEffect, useRef } from "react";

export function useLaunchHudInteractionState({
	openId,
	hudContentRef,
	hudBarRef,
	isHudDraggingRef,
	isWebcamPreviewDraggingRef,
	webcamPreviewDragStartRef,
}: {
	openId: string | null;
	hudContentRef: RefObject<HTMLDivElement | null>;
	hudBarRef: RefObject<HTMLDivElement | null>;
	isHudDraggingRef: RefObject<boolean>;
	isWebcamPreviewDraggingRef: RefObject<boolean>;
	webcamPreviewDragStartRef: RefObject<unknown>;
}) {
	const isMouseOverHudRef = useRef(false);
	const timeoutRef = useRef<NodeJS.Timeout | null>(null);
	const lastBoundsRef = useRef("");

	useEffect(() => {
		const publishInteractiveBounds = () => {
			const nodes = [
				hudContentRef.current,
				hudBarRef.current,
				...Array.from(
					document.querySelectorAll<HTMLElement>("[data-hud-interactive], .pointer-events-auto"),
				),
			].filter((node): node is HTMLElement => Boolean(node));
			const bounds = nodes
				.map((node) => node.getBoundingClientRect())
				.filter((rect) => rect.width > 0 && rect.height > 0)
				.map(({ left, top, right, bottom }) => ({ left, top, right, bottom }));
			const serialized = JSON.stringify(bounds);
			if (serialized !== lastBoundsRef.current) {
				lastBoundsRef.current = serialized;
				window.electronAPI?.hudOverlaySetInteractiveBounds?.(bounds);
			}
		};

		publishInteractiveBounds();
		const interval = window.setInterval(publishInteractiveBounds, 80);
		return () => window.clearInterval(interval);
	}, [hudBarRef, hudContentRef, openId]);

	useEffect(() => {
		if (openId !== null) {
			if (timeoutRef.current) clearTimeout(timeoutRef.current);
			window.electronAPI?.hudOverlaySetIgnoreMouse?.(false);
		} else {
			// Proactively check if we should ignore mouse when popover closes
			setTimeout(() => {
				if (!isMouseOverHudRef.current) {
					window.electronAPI?.hudOverlaySetIgnoreMouse?.(true);
				}
			}, 150);
		}
	}, [openId]);

	useEffect(() => {
		const handleMouseOver = (e: globalThis.MouseEvent) => {
			const target = e.target as HTMLElement | null;
			if (!target) return;
			const isInteractive = !!target.closest(".pointer-events-auto, [data-hud-interactive]");

			if (isInteractive) {
				isMouseOverHudRef.current = true;
				if (timeoutRef.current) clearTimeout(timeoutRef.current);
				window.electronAPI?.hudOverlaySetIgnoreMouse?.(false);
			} else if (openId === null) {
				isMouseOverHudRef.current = false;
				if (timeoutRef.current) clearTimeout(timeoutRef.current);
				timeoutRef.current = setTimeout(() => {
					if (
						openId === null &&
						!isHudDraggingRef.current &&
						!isWebcamPreviewDraggingRef.current &&
						!webcamPreviewDragStartRef.current &&
						!isMouseOverHudRef.current
					) {
						window.electronAPI?.hudOverlaySetIgnoreMouse?.(true);
					}
				}, 0);
			}
		};

		window.addEventListener("mouseover", handleMouseOver);
		return () => window.removeEventListener("mouseover", handleMouseOver);
	}, [openId, isHudDraggingRef, isWebcamPreviewDraggingRef, webcamPreviewDragStartRef]);

	const beginInteractiveHudAction = useCallback(() => {
		isMouseOverHudRef.current = true;
		window.electronAPI?.hudOverlaySetIgnoreMouse?.(false);
	}, []);

	const handleHudMouseEnter = useCallback(() => {
		isMouseOverHudRef.current = true;
		if (timeoutRef.current) clearTimeout(timeoutRef.current);
		window.electronAPI?.hudOverlaySetIgnoreMouse?.(false);
	}, []);

	const handleHudMouseLeave = useCallback(
		(event: MouseEvent<HTMLDivElement>) => {
			const nextTarget = event.relatedTarget;
			if (nextTarget instanceof Node && event.currentTarget.contains(nextTarget)) {
				return;
			}

			isMouseOverHudRef.current = false;

			if (timeoutRef.current) clearTimeout(timeoutRef.current);

			timeoutRef.current = setTimeout(() => {
				if (
					openId === null &&
					!isHudDraggingRef.current &&
					!isWebcamPreviewDraggingRef.current &&
					!webcamPreviewDragStartRef.current &&
					!isMouseOverHudRef.current
				) {
					window.electronAPI?.hudOverlaySetIgnoreMouse?.(true);
				}
			}, 0);
		},
		[openId, isHudDraggingRef, isWebcamPreviewDraggingRef, webcamPreviewDragStartRef],
	);

	return {
		handleHudMouseEnter,
		handleHudMouseLeave,
		beginInteractiveHudAction,
	};
}
