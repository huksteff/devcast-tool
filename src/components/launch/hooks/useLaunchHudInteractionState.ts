import { type MouseEvent, type RefObject, useCallback, useEffect, useRef } from "react";

export function useLaunchHudInteractionState({
	openId,
	mousePassthroughSupported,
	isHudDraggingRef,
	isWebcamPreviewDraggingRef,
	webcamPreviewDragStartRef,
}: {
	openId: string | null;
	mousePassthroughSupported: boolean;
	isHudDraggingRef: RefObject<boolean>;
	isWebcamPreviewDraggingRef: RefObject<boolean>;
	webcamPreviewDragStartRef: RefObject<unknown>;
}) {
	const isMouseOverHudRef = useRef(false);
	const timeoutRef = useRef<NodeJS.Timeout | null>(null);

	useEffect(() => {
		if (openId !== null) {
			if (timeoutRef.current) clearTimeout(timeoutRef.current);
			window.electronAPI?.hudOverlaySetIgnoreMouse?.(false);
		} else if (!mousePassthroughSupported) {
			// Compact-window platforms resize only for an actually open popover.
			// Hover must never resize the native window: doing so can synthesize a
			// mouseleave and make the HUD visibly jump between bounds.
			window.electronAPI?.hudOverlaySetIgnoreMouse?.(true);
		} else {
			// Proactively check if we should ignore mouse when popover closes
			setTimeout(() => {
				if (!isMouseOverHudRef.current) {
					window.electronAPI?.hudOverlaySetIgnoreMouse?.(true);
				}
			}, 150);
		}
	}, [mousePassthroughSupported, openId]);

	useEffect(() => {
		if (!mousePassthroughSupported) return;

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
	}, [
		mousePassthroughSupported,
		openId,
		isHudDraggingRef,
		isWebcamPreviewDraggingRef,
		webcamPreviewDragStartRef,
	]);

	const beginInteractiveHudAction = useCallback(() => {
		isMouseOverHudRef.current = true;
		window.electronAPI?.hudOverlaySetIgnoreMouse?.(false);
	}, []);

	const handleHudMouseEnter = useCallback(() => {
		isMouseOverHudRef.current = true;
		if (timeoutRef.current) clearTimeout(timeoutRef.current);
		if (mousePassthroughSupported) {
			window.electronAPI?.hudOverlaySetIgnoreMouse?.(false);
		}
	}, [mousePassthroughSupported]);

	const handleHudMouseLeave = useCallback(
		(event: MouseEvent<HTMLDivElement>) => {
			const nextTarget = event.relatedTarget;
			if (nextTarget instanceof Node && event.currentTarget.contains(nextTarget)) {
				return;
			}

			isMouseOverHudRef.current = false;
			if (!mousePassthroughSupported) return;

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
		[
			mousePassthroughSupported,
			openId,
			isHudDraggingRef,
			isWebcamPreviewDraggingRef,
			webcamPreviewDragStartRef,
		],
	);

	return {
		handleHudMouseEnter,
		handleHudMouseLeave,
		beginInteractiveHudAction,
	};
}
