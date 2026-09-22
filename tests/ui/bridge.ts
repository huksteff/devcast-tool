import type { Page } from "@playwright/test";
export async function installDesktopBridge(page: Page, videoFixture = "preview.mp4") {
	await page.addInitScript((videoFixture) => {
		const success = async () => ({ success: true });
		const subscribe = () => () => undefined;
		const source = {
			id: "screen:1:0",
			name: "Built-in Display",
			thumbnail: "",
			display_id: "1",
		};
		Object.assign(window, {
			electronAPI: {
				getAppSetting: () => null,
				setAppSetting: () => true,
				getExperimentalUpdatesEnabled: async () => false,
				getScreenRecordingPermissionStatus: async () => ({
					success: true,
					status: "granted",
				}),
				getAccessibilityPermissionStatus: async () => ({ success: true, trusted: true }),
				getPlatform: async () => "darwin",
				getWindowChrome: async () => ({ trafficLightsVisible: true }),
				onWindowChromeChanged: (
					callback: (chrome: { trafficLightsVisible: boolean }) => void,
				) => {
					const listener = (event: Event) => callback((event as CustomEvent).detail);
					window.addEventListener("test-window-chrome", listener);
					return () => window.removeEventListener("test-window-chrome", listener);
				},
				getAppVersion: async () => "1.4.0",
				getAnnouncements: async () => ({ success: true, announcements: [] }),
				loadCurrentProjectFile: async () => ({ success: false }),
				getCurrentRecordingSession: async () => ({ success: true, session: null }),
				getCurrentVideoPath: async () => ({
					success: true,
					path: `${location.origin}/tests/ui/fixtures/${videoFixture}`,
				}),
				getCursorTelemetry: async () => ({ success: true, samples: [] }),
				getVideoAudioFallbackPaths: async () => ({ success: true, paths: [] }),
				getWhisperSmallModelStatus: async () => ({ success: true, exists: false }),
				listProjectFiles: async () => ({ success: true, projects: [], entries: [] }),
				setCurrentVideoPath: success,
				finishRecordingImport: success,
				setCurrentRecordingSession: success,
				setHasUnsavedChanges: success,
				onMenuSaveProject: subscribe,
				onMenuSaveProjectAs: subscribe,
				onMenuLoadProject: subscribe,
				onRequestSaveBeforeClose: subscribe,
				onRecordingSessionChanged: subscribe,
				onWhisperSmallModelDownloadProgress: subscribe,
				getSelectedSource: async () => source,
				getSources: async () => [source],
				selectSource: success,
				onSelectedSourceChanged: subscribe,
				getRecordingsDirectory: async () => ({ success: true, path: "/recordings" }),
				getHudOverlayMousePassthroughSupported: async () => ({
					success: true,
					supported: true,
				}),
				getHudOverlayCaptureProtection: async () => ({
					success: true,
					enabled: false,
					supported: true,
				}),
				getRecordingAudioLabConfig: async () => ({ success: true, enabled: false }),
				getCountdownDelay: async () => ({ success: true, delay: 3 }),
				setCountdownDelay: success,
				getRecordingPreferences: async () => ({
					success: true,
					microphoneEnabled: false,
					webcamEnabled: false,
					systemAudioEnabled: true,
				}),
				setRecordingPreferences: success,
				onStopRecordingFromTray: subscribe,
				onRecordingStateChanged: subscribe,
				onRecordingInterrupted: subscribe,
				hudOverlayRendererReady: success,
				hudOverlaySetIgnoreMouse: success,
				hudOverlaySetWebcamPreviewVisible: success,
				getEditorMode: async () => false,
				onEditorModeChanged: subscribe,
				getActiveCountdown: async () => ({ success: true, seconds: 3 }),
				onCountdownTick: subscribe,
				cancelCountdown: async () => {
					document.documentElement.dataset.countdownCancelled = "true";
					return { success: true };
				},
				getCurrentUpdateToastPayload: async () => ({
					version: "1.4.1",
					detail: "A new version of Recordly is available.",
					phase: "available",
					delayMs: 60000,
					isPreview: true,
				}),
				onUpdateToastStateChanged: subscribe,
				dismissUpdateToast: async () => {
					document.documentElement.dataset.updateDismissed = "true";
					return { success: true };
				},
				...window.electronAPI,
			},
		});
	}, videoFixture);
}

/** Overrides can run before or after bridge defaults; Playwright does not order init scripts. */
export async function installDesktopBridgeOverrides<T = undefined>(
	page: Page,
	setup: (arg: T) => void,
	arg?: T,
) {
	await page.addInitScript({
		content: `window.electronAPI ??= {}; (${setup.toString()})(${JSON.stringify(arg) ?? "undefined"});`,
	});
}
