export interface HudOverlayTaskbarOptions {
	skipTaskbar: boolean;
	focusable: boolean;
}

export function getHudOverlayTaskbarOptions(_platform: NodeJS.Platform): HudOverlayTaskbarOptions {
	// A recording overlay must accept mouse input without activating its native
	// window. On Windows focusable:false sets WS_EX_NOACTIVATE (and hides the
	// taskbar entry); the tray provides access to the HUD instead.
	return {
		skipTaskbar: true,
		focusable: false,
	};
}
