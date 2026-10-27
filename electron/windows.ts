import fs from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { app, BrowserWindow, ipcMain } from "electron";
import { supportsHudCaptureProtection } from "../src/lib/hudCaptureProtection";
import { USER_DATA_PATH } from "./appPaths";
import {
	getHudOverlayWindowShape,
	getHudOverlayWindowBounds,
	getStableHudOverlayWindowBounds,
	resizeHudOverlayFallbackBounds,
	shouldExpandHudOverlayFallback,
} from "./hudOverlayBounds";
import { getHudOverlayTaskbarOptions } from "./hudOverlayWindowOptions";
import { getPackagedRendererBaseUrl } from "./rendererServer";

const electronWindowsDir = path.dirname(fileURLToPath(import.meta.url));
const nodeRequire = createRequire(import.meta.url);

const APP_ROOT = path.join(electronWindowsDir, "..");
const VITE_DEV_SERVER_URL = process.env["VITE_DEV_SERVER_URL"];
const RENDERER_DIST = path.join(APP_ROOT, "dist");
const WINDOW_ICON_FILENAME =
	process.platform === "darwin" ? "devcastmac-512.png" : "devcast-512.png";
const WINDOW_ICON_PATH = path.join(
	process.env.VITE_PUBLIC || RENDERER_DIST,
	"content",
	"icons",
	WINDOW_ICON_FILENAME,
);

let hudOverlayWindow: BrowserWindow | null = null;
let hudOverlayHiddenFromCapture = true;
let hudOverlayCaptureProtectionLoaded = false;
let hudOverlayFallbackExpanded = false;
let hudOverlayIgnoringMouse = true;
let hudOverlaySourceSelectionActive = false;
let hudOverlayMouseReassertTimer: NodeJS.Timeout | null = null;
let hudOverlayRecordingActive = false;
let hudOverlayWebcamPreviewVisible = false;
let countdownWindow: BrowserWindow | null = null;
let updateToastWindow: BrowserWindow | null = null;
let timelineWindow: BrowserWindow | null = null;
let timelineOwner: BrowserWindow | null = null;
let latestTimelineSnapshot: unknown = null;
let hudWasVisibleBeforeUpdateToast = false;

const HUD_OVERLAY_SETTINGS_FILE = path.join(USER_DATA_PATH, "hud-overlay-settings.json");
const HUD_EDGE_MARGIN_DIP = 16;
const UPDATE_TOAST_WIDTH = 420;
const UPDATE_TOAST_HEIGHT = 172;

function getEditorWindowQuery(): Record<string, string> {
	const query: Record<string, string> = {
		windowType: "editor",
	};

	if (process.env.DEVCAST_DEV_OPEN_RECORDING_INPUT) {
		query.devOpenInput = process.env.DEVCAST_DEV_OPEN_RECORDING_INPUT;
	}
	if (process.env.DEVCAST_DEV_OPEN_RECORDING_WEBCAM) {
		query.devOpenWebcam = process.env.DEVCAST_DEV_OPEN_RECORDING_WEBCAM;
	}

	if (process.env.DEVCAST_SMOKE_EXPORT === "1") {
		query.smokeExport = "1";
		if (process.env.DEVCAST_SMOKE_EXPORT_INPUT) {
			query.smokeInput = process.env.DEVCAST_SMOKE_EXPORT_INPUT;
		}
		if (process.env.DEVCAST_SMOKE_EXPORT_OUTPUT) {
			query.smokeOutput = process.env.DEVCAST_SMOKE_EXPORT_OUTPUT;
		}
		if (process.env.DEVCAST_SMOKE_EXPORT_USE_NATIVE === "1") {
			query.smokeUseNativeExport = "1";
		}
		if (process.env.DEVCAST_SMOKE_EXPORT_ENCODING_MODE) {
			query.smokeEncodingMode = process.env.DEVCAST_SMOKE_EXPORT_ENCODING_MODE;
		}
		if (process.env.DEVCAST_SMOKE_EXPORT_SHADOW_INTENSITY) {
			query.smokeShadowIntensity = process.env.DEVCAST_SMOKE_EXPORT_SHADOW_INTENSITY;
		}
		if (process.env.DEVCAST_SMOKE_EXPORT_WEBCAM_INPUT) {
			query.smokeWebcamInput = process.env.DEVCAST_SMOKE_EXPORT_WEBCAM_INPUT;
		}
		if (process.env.DEVCAST_SMOKE_EXPORT_WEBCAM_SHADOW) {
			query.smokeWebcamShadow = process.env.DEVCAST_SMOKE_EXPORT_WEBCAM_SHADOW;
		}
		if (process.env.DEVCAST_SMOKE_EXPORT_WEBCAM_SIZE) {
			query.smokeWebcamSize = process.env.DEVCAST_SMOKE_EXPORT_WEBCAM_SIZE;
		}
		if (process.env.DEVCAST_SMOKE_EXPORT_PIPELINE) {
			query.smokePipelineModel = process.env.DEVCAST_SMOKE_EXPORT_PIPELINE;
		}
		if (process.env.DEVCAST_SMOKE_EXPORT_BACKEND) {
			query.smokeBackendPreference = process.env.DEVCAST_SMOKE_EXPORT_BACKEND;
		}
		if (process.env.DEVCAST_SMOKE_EXPORT_RENDER_BACKEND) {
			query.smokeRenderBackend = process.env.DEVCAST_SMOKE_EXPORT_RENDER_BACKEND;
		}
		if (process.env.DEVCAST_SMOKE_EXPORT_MAX_ENCODE_QUEUE) {
			query.smokeMaxEncodeQueue = process.env.DEVCAST_SMOKE_EXPORT_MAX_ENCODE_QUEUE;
		}
		if (process.env.DEVCAST_SMOKE_EXPORT_MAX_DECODE_QUEUE) {
			query.smokeMaxDecodeQueue = process.env.DEVCAST_SMOKE_EXPORT_MAX_DECODE_QUEUE;
		}
		if (process.env.DEVCAST_SMOKE_EXPORT_MAX_PENDING_FRAMES) {
			query.smokeMaxPendingFrames = process.env.DEVCAST_SMOKE_EXPORT_MAX_PENDING_FRAMES;
		}
		if (process.env.DEVCAST_SMOKE_EXPORT_PROJECT) {
			query.smokeProject = process.env.DEVCAST_SMOKE_EXPORT_PROJECT;
		}
		if (process.env.DEVCAST_SMOKE_EXPORT_QUALITY) {
			query.smokeQuality = process.env.DEVCAST_SMOKE_EXPORT_QUALITY;
		}
		if (process.env.DEVCAST_SMOKE_EXPORT_FPS) {
			query.smokeFps = process.env.DEVCAST_SMOKE_EXPORT_FPS;
		}
	}

	return query;
}

export function isHudOverlayMousePassthroughSupported(): boolean {
	// macOS provides reliable forwarded pointer events for transparent overlays.
	// Windows does not: WS_EX_TRANSPARENT can remain stuck after activation
	// changes. Use the compact/expanded real-window HUD there instead.
	return process.platform === "darwin";
}

function loadHudOverlayCaptureProtectionSetting(): boolean {
	if (hudOverlayCaptureProtectionLoaded) {
		return hudOverlayHiddenFromCapture;
	}

	hudOverlayCaptureProtectionLoaded = true;

	try {
		if (!fs.existsSync(HUD_OVERLAY_SETTINGS_FILE)) {
			return hudOverlayHiddenFromCapture;
		}

		const raw = fs.readFileSync(HUD_OVERLAY_SETTINGS_FILE, "utf-8");
		const parsed = JSON.parse(raw) as { hiddenFromCapture?: unknown };
		if (typeof parsed.hiddenFromCapture === "boolean") {
			hudOverlayHiddenFromCapture = parsed.hiddenFromCapture;
		}
	} catch {
		// Ignore settings read failures and fall back to defaults.
	}

	return hudOverlayHiddenFromCapture;
}

export function getHudOverlayCaptureProtectionEnabled(): boolean {
	return loadHudOverlayCaptureProtectionSetting();
}

function applyHudOverlayCaptureProtectionToWindow(hud: BrowserWindow, enabled: boolean): void {
	if (!supportsHudCaptureProtection(process.platform)) {
		return;
	}

	try {
		hud.setContentProtection(enabled);
	} catch (error) {
		console.warn("Failed to apply HUD capture protection:", error);
	}
}

export function reassertHudOverlayCaptureProtection(): boolean {
	const enabled = loadHudOverlayCaptureProtectionSetting();
	const hud = getHudOverlayWindow();
	if (!hud) {
		return enabled;
	}

	applyHudOverlayCaptureProtectionToWindow(hud, enabled);

	return enabled;
}

function persistHudOverlayCaptureProtectionSetting(enabled: boolean): void {
	try {
		fs.writeFileSync(
			HUD_OVERLAY_SETTINGS_FILE,
			JSON.stringify({ hiddenFromCapture: enabled }, null, 2),
			"utf-8",
		);
	} catch {
		// Ignore settings write failures and keep runtime state working.
	}
}

function getScreen() {
	if (!app.isReady()) {
		throw new Error(
			"getScreen() called before app is ready. Ensure all screen access happens after app.whenReady().",
		);
	}
	return nodeRequire("electron").screen as typeof import("electron").screen;
}

function getHudOverlayDisplay() {
	const hudWindow = getHudOverlayWindow();
	if (hudWindow) {
		return getScreen().getDisplayMatching(hudWindow.getBounds());
	}
	return getScreen().getPrimaryDisplay();
}

function getHudOverlayBounds() {
	const { workArea } = getHudOverlayDisplay();
	if (usesStableHudOverlayShape()) {
		return getStableHudOverlayWindowBounds(workArea);
	}

	const fallbackExpanded = shouldExpandHudOverlayFallback({
		fallbackExpanded: hudOverlayFallbackExpanded,
		recordingActive: hudOverlayRecordingActive,
		webcamPreviewVisible: hudOverlayWebcamPreviewVisible,
	});
	return getHudOverlayWindowBounds(
		workArea,
		isHudOverlayMousePassthroughSupported(),
		fallbackExpanded,
	);
}

function usesStableHudOverlayShape(): boolean {
	return process.platform === "win32" && !isHudOverlayMousePassthroughSupported();
}

function isSameBounds(
	left: { x: number; y: number; width: number; height: number },
	right: { x: number; y: number; width: number; height: number },
): boolean {
	return (
		left.x === right.x &&
		left.y === right.y &&
		left.width === right.width &&
		left.height === right.height
	);
}

function applyHudOverlayShape(target = hudOverlayWindow): void {
	if (!usesStableHudOverlayShape() || !target || target.isDestroyed()) {
		return;
	}

	const expanded = shouldExpandHudOverlayFallback({
		fallbackExpanded: hudOverlayFallbackExpanded,
		recordingActive: hudOverlayRecordingActive,
		webcamPreviewVisible: hudOverlayWebcamPreviewVisible,
	});
	target.setShape(getHudOverlayWindowShape(target.getBounds(), expanded));
}

function applyHudOverlayBounds() {
	if (!hudOverlayWindow || hudOverlayWindow.isDestroyed()) {
		return;
	}
	const nextBounds = getHudOverlayBounds();
	if (!isSameBounds(hudOverlayWindow.getBounds(), nextBounds)) {
		hudOverlayWindow.setBounds(nextBounds, false);
	}
	applyHudOverlayShape();

	positionUpdateToastWindow();
	if (!hudOverlayWindow.isVisible()) {
		return;
	}
	hudOverlayWindow.moveTop();
}

function getUpdateToastBounds() {
	const hudWindow = getHudOverlayWindow();
	if (hudWindow) {
		const hudBounds = hudWindow.getBounds();
		const display = getScreen().getDisplayMatching(hudBounds);
		const { workArea } = display;
		const x = Math.round(workArea.x + (workArea.width - UPDATE_TOAST_WIDTH) / 2);
		const y = Math.round(
			workArea.y + workArea.height - UPDATE_TOAST_HEIGHT - HUD_EDGE_MARGIN_DIP,
		);

		return {
			x,
			y,
			width: UPDATE_TOAST_WIDTH,
			height: UPDATE_TOAST_HEIGHT,
		};
	}

	const primaryDisplay = getScreen().getPrimaryDisplay();
	const { workArea } = primaryDisplay;
	return {
		x: Math.round(workArea.x + (workArea.width - UPDATE_TOAST_WIDTH) / 2),
		y: Math.round(workArea.y + workArea.height - UPDATE_TOAST_HEIGHT - HUD_EDGE_MARGIN_DIP),
		width: UPDATE_TOAST_WIDTH,
		height: UPDATE_TOAST_HEIGHT,
	};
}

function positionUpdateToastWindow() {
	if (!updateToastWindow || updateToastWindow.isDestroyed()) {
		return;
	}

	updateToastWindow.setBounds(getUpdateToastBounds(), false);
	updateToastWindow.moveTop();
}

function setHudOverlayFallbackExpanded(expanded: boolean) {
	if (hudOverlayRecordingActive) {
		hudOverlayFallbackExpanded = false;
		return;
	}

	hudOverlayFallbackExpanded = expanded;
	if (
		!hudOverlayWindow ||
		hudOverlayWindow.isDestroyed() ||
		isHudOverlayMousePassthroughSupported()
	) {
		return;
	}
	if (usesStableHudOverlayShape()) {
		// Do not resize the transparent Windows BrowserWindow when a popover
		// opens. Changing only the native input/paint region avoids the DWM flash.
		applyHudOverlayShape();
		if (hudOverlayWindow.isVisible()) {
			hudOverlayWindow.moveTop();
		}
		return;
	}

	const { workArea } = getHudOverlayDisplay();
	const nextBounds = resizeHudOverlayFallbackBounds(
		workArea,
		hudOverlayWindow.getBounds(),
		expanded,
	);
	hudOverlayWindow.setBounds(nextBounds, false);
	positionUpdateToastWindow();
	if (hudOverlayWindow.isVisible()) {
		hudOverlayWindow.moveTop();
	}
}

function setHudOverlayMousePassthrough(ignore: boolean) {
	hudOverlayIgnoringMouse =
		hudOverlaySourceSelectionActive && !hudOverlayRecordingActive ? true : ignore;

	if (hudOverlayMouseReassertTimer) {
		clearTimeout(hudOverlayMouseReassertTimer);
		hudOverlayMouseReassertTimer = null;
	}

	if (!hudOverlayWindow || hudOverlayWindow.isDestroyed()) {
		return;
	}

	if (hudOverlayRecordingActive) {
		hudOverlayFallbackExpanded = false;
		applyHudOverlayBounds();
	}

	if (!isHudOverlayMousePassthroughSupported()) {
		if (process.platform !== "linux") {
			setHudOverlayFallbackExpanded(!hudOverlayIgnoringMouse);
		}
		hudOverlayWindow.setIgnoreMouseEvents(false);
		return;
	}

	if (hudOverlayIgnoringMouse) {
		hudOverlayWindow.setIgnoreMouseEvents(true, { forward: true });
		return;
	}

	hudOverlayWindow.setIgnoreMouseEvents(false);
}

ipcMain.on("hud-overlay-set-ignore-mouse", (_event, ignore: boolean) => {
	setHudOverlayMousePassthrough(Boolean(ignore));
});

ipcMain.on("hud-overlay-set-source-selection-active", (_event, active: boolean) => {
	hudOverlaySourceSelectionActive = Boolean(active);
	if (hudOverlaySourceSelectionActive) {
		hudOverlayFallbackExpanded = false;
		applyHudOverlayBounds();
		setHudOverlayMousePassthrough(true);
		return;
	}

	setHudOverlayMousePassthrough(true);
});

// Keep compatibility with existing drag IPC/state.
let hudUserPosition: { x: number; y: number } | null = null;
let hudDragOffset: { x: number; y: number } | null = null;
let hudDragLastCursor: { x: number; y: number } | null = null;
let hudDragFixedSize: { width: number; height: number } | null = null;

ipcMain.on("hud-overlay-drag", (_event, phase: string, screenX: number, screenY: number) => {
	if (!hudOverlayWindow || hudOverlayWindow.isDestroyed()) return;

	// On Linux the compositor (especially Wayland) refuses programmatic window
	// placement, so BrowserWindow.setBounds() with x/y is silently ignored and
	// the HUD appears "stuck".  The renderer marks the drag handle as
	// -webkit-app-region: drag on Linux, letting the OS move the window for us.
	// The resulting position is captured by the win.on("moved", ...) listener
	// below so `hudUserPosition` stays in sync.
	if (process.platform === "linux") {
		return;
	}

	if (phase === "start") {
		const bounds = hudOverlayWindow.getBounds();
		hudDragOffset = { x: screenX - bounds.x, y: screenY - bounds.y };
		hudDragLastCursor = { x: screenX, y: screenY };
		hudDragFixedSize = { width: bounds.width, height: bounds.height };
	} else if (phase === "move" && hudDragOffset) {
		if (
			hudDragLastCursor &&
			hudDragLastCursor.x === screenX &&
			hudDragLastCursor.y === screenY
		) {
			return;
		}

		hudDragLastCursor = { x: screenX, y: screenY };
		const targetX = Math.round(screenX - hudDragOffset.x);
		const targetY = Math.round(screenY - hudDragOffset.y);
		const fixedWidth = hudDragFixedSize?.width ?? hudOverlayWindow.getBounds().width;
		const fixedHeight = hudDragFixedSize?.height ?? hudOverlayWindow.getBounds().height;
		hudOverlayWindow.setBounds(
			{
				x: targetX,
				y: targetY,
				width: fixedWidth,
				height: fixedHeight,
			},
			false,
		);
	} else if (phase === "end") {
		const finalBounds = hudOverlayWindow.getBounds();
		hudUserPosition = { x: finalBounds.x, y: finalBounds.y };

		hudDragOffset = null;
		hudDragLastCursor = null;
		hudDragFixedSize = null;
	}
});

ipcMain.on("hud-overlay-hide", () => {
	if (hudOverlayWindow && !hudOverlayWindow.isDestroyed()) {
		hudOverlayWindow.minimize();
	}
});

ipcMain.handle("get-hud-overlay-capture-protection", () => {
	const enabled = loadHudOverlayCaptureProtectionSetting();

	return {
		success: true,
		enabled,
	};
});

ipcMain.handle("get-hud-overlay-mouse-passthrough-supported", () => {
	return {
		success: true,
		supported: isHudOverlayMousePassthroughSupported(),
	};
});

ipcMain.on("hud-overlay-set-webcam-preview-visible", (_event, visible: boolean) => {
	const nextVisible = Boolean(visible);
	if (hudOverlayWebcamPreviewVisible === nextVisible) {
		return;
	}

	hudOverlayWebcamPreviewVisible = nextVisible;
	if (hudOverlayRecordingActive) {
		applyHudOverlayBounds();
	}
});

ipcMain.handle("set-hud-overlay-capture-protection", (_event, enabled: boolean) => {
	loadHudOverlayCaptureProtectionSetting();
	hudOverlayHiddenFromCapture = Boolean(enabled);
	persistHudOverlayCaptureProtectionSetting(hudOverlayHiddenFromCapture);

	reassertHudOverlayCaptureProtection();

	return {
		success: true,
		enabled: hudOverlayHiddenFromCapture,
	};
});

const editorWindows = new Set<BrowserWindow>();
function notifyEditorMode() {
	if (hudOverlayWindow && !hudOverlayWindow.webContents.isDestroyed()) {
		hudOverlayWindow.webContents.send("editor-mode-changed", editorWindows.size > 0);
	}
}
ipcMain.handle("get-editor-mode", () => editorWindows.size > 0);

export function createHudOverlayWindow(): BrowserWindow {
	const perfStart = Date.now();
	loadHudOverlayCaptureProtectionSetting();
	hudOverlayFallbackExpanded = false;
	hudOverlayWebcamPreviewVisible = false;
	const initialBounds = getHudOverlayBounds();
	let hasShownHudWindow = false;

	const win = new BrowserWindow({
		width: initialBounds.width,
		height: initialBounds.height,
		x: initialBounds.x,
		y: initialBounds.y,
		frame: false,
		transparent: true,
		backgroundColor: "#00000000",
		resizable: false,
		alwaysOnTop: true,
		// Mouse controls must not deactivate the application being recorded.
		...getHudOverlayTaskbarOptions(process.platform),
		hasShadow: false,
		show: false,
		webPreferences: {
			preload: path.join(electronWindowsDir, "preload.cjs"),
			nodeIntegration: false,
			contextIsolation: true,
			webSecurity: false,
			backgroundThrottling: false,
		},
	});
	// Keep the recording controls and webcam above normal and full-screen apps.
	// Transparent regions remain click-through via setIgnoreMouseEvents().
	win.setAlwaysOnTop(true, "screen-saver");
	if (process.platform === "darwin") {
		win.setVisibleOnAllWorkspaces(true, {
			visibleOnFullScreen: true,
			skipTransformProcessType: true,
		});
	}

	const showHudWindow = () => {
		if (hasShownHudWindow || win.isDestroyed()) {
			return;
		}
		if (
			updateToastWindow &&
			!updateToastWindow.isDestroyed() &&
			updateToastWindow.isVisible()
		) {
			hudWasVisibleBeforeUpdateToast = true;
			return;
		}
		hasShownHudWindow = true;
		applyHudOverlayShape(win);
		// Showing or changing native window state can recreate platform window
		// flags. Reassert capture protection on both sides of the transition.
		applyHudOverlayCaptureProtectionToWindow(win, hudOverlayHiddenFromCapture);
		if (process.platform === "win32") {
			// Show the non-activating recording controls above the current app.
			win.showInactive();
		} else {
			win.show();
		}
		win.moveTop();
		applyHudOverlayCaptureProtectionToWindow(win, hudOverlayHiddenFromCapture);
		if (process.platform === "win32" && isHudOverlayMousePassthroughSupported()) {
			win.setIgnoreMouseEvents(false);
			setTimeout(() => {
				if (!win.isDestroyed()) {
					setHudOverlayMousePassthrough(hudOverlayIgnoringMouse);
				}
			}, 50);
		}
	};

	applyHudOverlayCaptureProtectionToWindow(win, hudOverlayHiddenFromCapture);
	win.on("show", () => {
		if (!win.isDestroyed()) {
			applyHudOverlayCaptureProtectionToWindow(win, hudOverlayHiddenFromCapture);
			applyHudOverlayShape(win);
			if (!isHudOverlayMousePassthroughSupported()) win.setIgnoreMouseEvents(false);
		}
	});

	if (isHudOverlayMousePassthroughSupported()) {
		// Keep the HUD interactive from its first frame. On Windows this avoids
		// depending on a future forwarded hover event to restore input.
		hudOverlayIgnoringMouse = false;
		win.setIgnoreMouseEvents(false);
	}


	win.webContents.on("did-finish-load", () => {
		console.log(`[PERF:MAIN] HUD Window: did-finish-load in ${Date.now() - perfStart}ms`);
		win?.webContents.send("main-process-message", new Date().toLocaleString());
		// Safety fallback if renderer-ready signal never arrives.
		setTimeout(() => {
			showHudWindow();
		}, 1800);
	});

	// Safety net: on Linux the renderer may fail to fire did-finish-load
	// (for example due to GPU/VAAPI startup issues). Show the window after
	// ready-to-show as a fallback so the HUD still appears.
	win.once("ready-to-show", () => {
		setTimeout(() => {
			if (!win.isDestroyed() && !win.isVisible()) {
				showHudWindow();
			}
		}, 500);
	});

	const handleHudRendererReady = () => {
		if (!win.isDestroyed()) {
			console.log(`[PERF:MAIN] HUD Window: renderer-ready in ${Date.now() - perfStart}ms`);
			showHudWindow();
		}
	};
	ipcMain.on("hud-overlay-renderer-ready", handleHudRendererReady);

	hudOverlayWindow = win;

	// On Linux the HUD is dragged by the OS via -webkit-app-region (Wayland
	// forbids client-side positioning). Mirror moved bounds into drag state.
	if (process.platform === "linux") {
		win.on("moved", () => {
			if (win.isDestroyed()) return;
			const { x, y } = win.getBounds();
			hudUserPosition = { x, y };
		});
	}

	// Reset the user's saved HUD position when displays change so the bar
	// doesn't end up stranded off-screen after a monitor is disconnected.
	const screen = getScreen();
	const handleDisplayRemoved = () => {
		hudUserPosition = null;
	};
	const handleDisplayMetricsChanged = () => {
		if (hudUserPosition) {
			const displays = screen.getAllDisplays();
			const onScreen = displays.some(
				(d) =>
					hudUserPosition!.x >= d.workArea.x &&
					hudUserPosition!.x < d.workArea.x + d.workArea.width &&
					hudUserPosition!.y >= d.workArea.y &&
					hudUserPosition!.y < d.workArea.y + d.workArea.height,
			);
			if (!onScreen) {
				hudUserPosition = null;
			}
		}
		applyHudOverlayBounds();
	};
	screen.on("display-removed", handleDisplayRemoved);
	screen.on("display-metrics-changed", handleDisplayMetricsChanged);

	win.on("closed", () => {
		ipcMain.removeListener("hud-overlay-renderer-ready", handleHudRendererReady);
		screen.removeListener("display-removed", handleDisplayRemoved);
		screen.removeListener("display-metrics-changed", handleDisplayMetricsChanged);
		if (hudOverlayWindow === win) {
			hudOverlayWindow = null;
		}
	});

	if (VITE_DEV_SERVER_URL) {
		win.loadURL(VITE_DEV_SERVER_URL + "?windowType=hud-overlay");
	} else {
		win.loadFile(path.join(RENDERER_DIST, "index.html"), {
			query: { windowType: "hud-overlay" },
		});
	}

	return win;
}

export function getHudOverlayWindow(): BrowserWindow | null {
	return hudOverlayWindow && !hudOverlayWindow.isDestroyed() ? hudOverlayWindow : null;
}

export function createTimelineWindow(): BrowserWindow {
	if (timelineWindow && !timelineWindow.isDestroyed()) {
		timelineWindow.show();
		timelineWindow.focus();
		return timelineWindow;
	}

	const win = new BrowserWindow({
		width: 1280,
		height: 460,
		minWidth: 720,
		minHeight: 260,
		...(process.platform !== "darwin" && { icon: WINDOW_ICON_PATH }),
		autoHideMenuBar: true,
		resizable: true,
		show: false,
		title: "DevCast Timeline",
		backgroundColor: "#181a1f",
		webPreferences: {
			preload: path.join(electronWindowsDir, "preload.cjs"),
			nodeIntegration: false,
			contextIsolation: true,
			webSecurity: false,
			backgroundThrottling: false,
		},
	});

	timelineWindow = win;
	win.once("ready-to-show", () => win.show());
	win.once("closed", () => {
		if (timelineWindow === win) {
			timelineWindow = null;
			for (const editor of getTimelineEditorWindows()) {
				if (!editor.isDestroyed())
					editor.webContents.send("timeline-window:message", { type: "window-closed" });
			}
		}
	});
	if (VITE_DEV_SERVER_URL) {
		void win.loadURL(`${VITE_DEV_SERVER_URL}?windowType=timeline-window`);
	} else {
		void win.loadFile(path.join(RENDERER_DIST, "index.html"), {
			query: { windowType: "timeline-window" },
		});
	}
	return win;
}

function getTimelineEditorWindows(): BrowserWindow[] {
	return timelineOwner && !timelineOwner.isDestroyed() ? [timelineOwner] : [];
}

ipcMain.handle("timeline-window:open", (event) => {
	const owner = BrowserWindow.fromWebContents(event.sender);
	if (!owner || !editorWindows.has(owner)) return { success: false };
	if (timelineOwner !== owner) {
		if (timelineWindow && !timelineWindow.isDestroyed()) timelineWindow.close();
		timelineOwner = owner;
		latestTimelineSnapshot = null;
		owner.once("closed", () => {
			if (timelineOwner !== owner) return;
			if (timelineWindow && !timelineWindow.isDestroyed()) timelineWindow.close();
			timelineOwner = null;
			latestTimelineSnapshot = null;
		});
	}
	createTimelineWindow();
	return { success: true };
});

ipcMain.handle("timeline-window:close", () => {
	if (timelineWindow && !timelineWindow.isDestroyed()) timelineWindow.close();
	return { success: true };
});

ipcMain.on("timeline-window:message", (event, message: unknown) => {
	const sender = BrowserWindow.fromWebContents(event.sender);
	if (sender && sender === timelineWindow) {
		const childWindow = timelineWindow;
		if (
			message &&
			typeof message === "object" &&
			"type" in message &&
			message.type === "snapshot-request" &&
			latestTimelineSnapshot
		) {
			childWindow?.webContents.send("timeline-window:message", latestTimelineSnapshot);
		}
		for (const editor of getTimelineEditorWindows()) {
			if (!editor.isDestroyed()) editor.webContents.send("timeline-window:message", message);
		}
		return;
	}
	if (!sender || sender !== timelineOwner) return;
	if (
		message &&
		typeof message === "object" &&
		"type" in message &&
		message.type === "snapshot"
	) {
		latestTimelineSnapshot = message;
	}
	if (timelineWindow && !timelineWindow.isDestroyed()) {
		timelineWindow.webContents.send("timeline-window:message", message);
	}
});

/**
 * Re-initialise the HUD overlay's mouse passthrough state.
 *
 * On Windows 11+, any new BrowserWindow appearing (even focusable:false ones
 * like the source highlight overlay) can silently corrupt the
 * WS_EX_TRANSPARENT flag that backs setIgnoreMouseEvents forwarding.  Call
 * this after any operation that creates or destroys a sibling window so that
 * hover detection on the HUD is immediately restored without requiring the
 * user to move their mouse over the bar.
 */
export function reassertHudOverlayMousePassthrough(): void {
	if (process.platform !== "win32") {
		return;
	}

	const hud = getHudOverlayWindow();
	if (!hud) {
		return;
	}
	// Native modal dialogs may leave the non-activating sibling HUD disabled.
	// Mouse flags and its input shape cannot make a disabled HWND clickable.
	hud.setEnabled(true);
	if (!isHudOverlayMousePassthroughSupported()) {
		// Windows uses a shaped input region. Reapply it after sibling windows
		// close too; the forwarded-mouse recovery below never runs on Windows.
		applyHudOverlayBounds();
		hud.setIgnoreMouseEvents(false);
		applyHudOverlayShape(hud);
		if (hud.isVisible()) hud.moveTop();
		if (hudOverlayMouseReassertTimer) clearTimeout(hudOverlayMouseReassertTimer);
		hudOverlayMouseReassertTimer = setTimeout(() => {
			hudOverlayMouseReassertTimer = null;
			if (hud.isDestroyed()) return;
			hud.setEnabled(true);
			hud.setIgnoreMouseEvents(false);
			applyHudOverlayShape(hud);
		}, 50);
		return;
	}

	// Toggle off then back on so the native WS_EX_TRANSPARENT flag is fully
	// re-initialised rather than merely re-asserted in a potentially broken state.
	hud.setIgnoreMouseEvents(false);
	if (hudOverlayMouseReassertTimer) {
		clearTimeout(hudOverlayMouseReassertTimer);
	}
	hudOverlayMouseReassertTimer = setTimeout(() => {
		hudOverlayMouseReassertTimer = null;
		if (!hud.isDestroyed()) {
			setHudOverlayMousePassthrough(hudOverlayIgnoringMouse);
		}
	}, 50);
}

export function setHudOverlayRecordingActive(recording: boolean): void {
	hudOverlayRecordingActive = Boolean(recording);
	hudOverlayFallbackExpanded = false;
	applyHudOverlayBounds();
	reassertHudOverlayCaptureProtection();
	// Start in passthrough mode. Forwarded pointer movement lets the renderer
	// make the visible HUD controls interactive when the pointer reaches them,
	// while transparent parts never block the recorded application.
	setHudOverlayMousePassthrough(true);
}

export function createUpdateToastWindow(): BrowserWindow {
	const initialBounds = getUpdateToastBounds();

	const win = new BrowserWindow({
		width: initialBounds.width,
		height: initialBounds.height,
		x: initialBounds.x,
		y: initialBounds.y,
		frame: false,
		transparent: true,
		resizable: false,
		alwaysOnTop: true,
		skipTaskbar: true,
		hasShadow: false,
		show: false,
		focusable: true,
		backgroundColor: "#00000000",
		webPreferences: {
			preload: path.join(electronWindowsDir, "preload.cjs"),
			nodeIntegration: false,
			contextIsolation: true,
			backgroundThrottling: false,
		},
	});

	if (process.platform === "darwin") {
		win.setAlwaysOnTop(true, "status");
	}

	win.setVisibleOnAllWorkspaces(true, {
		visibleOnFullScreen: true,
		// Keep DevCast a foreground application so macOS does not temporarily
		// remove its Dock icon while showing an overlay window.
		skipTransformProcessType: process.platform === "darwin",
	});
	updateToastWindow = win;

	win.on("closed", () => {
		if (updateToastWindow === win) {
			updateToastWindow = null;
		}
		restoreHudAfterUpdateToast();
	});

	if (VITE_DEV_SERVER_URL) {
		win.loadURL(VITE_DEV_SERVER_URL + "?windowType=update-toast");
	} else {
		win.loadFile(path.join(RENDERER_DIST, "index.html"), {
			query: { windowType: "update-toast" },
		});
	}

	return win;
}

export function getUpdateToastWindow(): BrowserWindow | null {
	return updateToastWindow && !updateToastWindow.isDestroyed() ? updateToastWindow : null;
}

export function showUpdateToastWindow(): BrowserWindow {
	const win = getUpdateToastWindow() ?? createUpdateToastWindow();
	const hud = getHudOverlayWindow();
	if (!win.isVisible()) {
		hudWasVisibleBeforeUpdateToast = Boolean(hud?.isVisible());
	}
	if (hud?.isVisible()) {
		hud.hide();
	}
	positionUpdateToastWindow();
	if (!win.isVisible()) {
		if (process.platform === "win32") {
			win.show();
		} else {
			win.showInactive();
		}
	}
	win.moveTop();

	return win;
}

function restoreHudAfterUpdateToast(): void {
	if (!hudWasVisibleBeforeUpdateToast) {
		return;
	}

	hudWasVisibleBeforeUpdateToast = false;
	const hud = getHudOverlayWindow();
	if (!hud) {
		return;
	}

	if (process.platform === "win32") {
		hud.showInactive();
	} else {
		hud.show();
	}
	hud.moveTop();
	setHudOverlayMousePassthrough(hudOverlayIgnoringMouse);
}

export function hideUpdateToastWindow(): void {
	if (updateToastWindow && !updateToastWindow.isDestroyed()) {
		updateToastWindow.hide();
	}
	restoreHudAfterUpdateToast();
}

function loadPackagedEditorWindow(win: BrowserWindow) {
	const query = getEditorWindowQuery();
	const queryString = new URLSearchParams(query).toString();
	const indexHtmlPath = path.join(RENDERER_DIST, "index.html");
	const packagedRendererBaseUrl = getPackagedRendererBaseUrl();
	const webContents = win.webContents;

	const loadFromFile = () => {
		if (win.isDestroyed()) {
			return;
		}

		console.log("[editor-window] load-file", indexHtmlPath);
		void win.loadFile(indexHtmlPath, { query });
	};

	if (!packagedRendererBaseUrl) {
		loadFromFile();
		return;
	}

	const targetUrl = `${packagedRendererBaseUrl}/?${queryString}`;
	let settled = false;
	let timeoutId: NodeJS.Timeout | null = setTimeout(() => {
		fallbackToFile("load-timeout");
	}, 5000);

	const clearTimeoutIfNeeded = () => {
		if (timeoutId) {
			clearTimeout(timeoutId);
			timeoutId = null;
		}
	};

	const detachLoadListeners = () => {
		clearTimeoutIfNeeded();
		if (webContents.isDestroyed()) {
			return;
		}

		webContents.removeListener("did-fail-load", handleDidFailLoad);
		webContents.removeListener("did-finish-load", handleDidFinishLoad);
	};

	const fallbackToFile = (reason: string, details?: Record<string, unknown>) => {
		if (settled || win.isDestroyed()) {
			return;
		}

		settled = true;
		detachLoadListeners();
		console.warn("[editor-window] packaged renderer URL failed, falling back to file", {
			reason,
			targetUrl,
			...details,
		});
		loadFromFile();
	};

	const handleDidFailLoad = (
		_event: Electron.Event,
		errorCode: number,
		errorDescription: string,
		validatedURL: string,
		isMainFrame: boolean,
	) => {
		if (!isMainFrame || validatedURL !== targetUrl) {
			return;
		}

		fallbackToFile("did-fail-load", {
			errorCode,
			errorDescription,
			validatedURL,
		});
	};

	const handleDidFinishLoad = () => {
		if (webContents.getURL() !== targetUrl) {
			return;
		}

		settled = true;
		detachLoadListeners();
	};

	webContents.on("did-fail-load", handleDidFailLoad);
	webContents.on("did-finish-load", handleDidFinishLoad);
	win.once("closed", clearTimeoutIfNeeded);

	console.log("[editor-window] load-url", targetUrl);
	void win.loadURL(targetUrl).catch((error) => {
		fallbackToFile("load-url-rejected", {
			error: error instanceof Error ? error.message : String(error),
		});
	});
}

export function createEditorWindow(): BrowserWindow {
	const perfStart = Date.now();
	console.log("[PERF:MAIN] createEditorWindow: STARTED");
	const isMac = process.platform === "darwin";
	const { workArea, workAreaSize } = getScreen().getPrimaryDisplay();
	const initialWidth = isMac ? Math.round(workAreaSize.width * 0.85) : workArea.width;
	const initialHeight = isMac ? Math.round(workAreaSize.height * 0.85) : workArea.height;

	const win = new BrowserWindow({
		width: initialWidth,
		height: initialHeight,
		...(!isMac && {
			x: workArea.x,
			y: workArea.y,
		}),
		minWidth: 800,
		minHeight: 600,
		...(process.platform !== "darwin" && {
			icon: WINDOW_ICON_PATH,
		}),
		...(isMac && {
			titleBarStyle: "hiddenInset",
			trafficLightPosition: { x: 16, y: 20 },
		}),
		autoHideMenuBar: !isMac,
		transparent: false,
		resizable: true,
		alwaysOnTop: false,
		skipTaskbar: false,
		title: "DevCast",
		show: false,
		backgroundColor: "#000000",
		webPreferences: {
			preload: path.join(electronWindowsDir, "preload.cjs"),
			nodeIntegration: false,
			contextIsolation: true,
			webSecurity: false,
			backgroundThrottling: false,
		},
	});

	editorWindows.add(win);
	notifyEditorMode();
	win.once("closed", () => {
		editorWindows.delete(win);
		notifyEditorMode();
		// Destroying a sibling BrowserWindow can leave the transparent HUD with a
		// stale WS_EX_TRANSPARENT state on Windows 11. Rebuild the passthrough
		// state after the editor's native window has actually gone away so hover
		// forwarding (and therefore the HUD buttons) starts working immediately.
		reassertHudOverlayMousePassthrough();
	});

	const publishWindowChrome = () => {
		if (!win.isDestroyed())
			win.webContents.send("window-chrome-changed", {
				trafficLightsVisible: isMac && !win.isFullScreen() && !win.isSimpleFullScreen(),
			});
	};
	win.on("enter-full-screen", publishWindowChrome);
	win.on("leave-full-screen", publishWindowChrome);
	win.on("resize", publishWindowChrome);
	win.webContents.on("did-finish-load", publishWindowChrome);

	win.once("ready-to-show", () => {
		console.log(`[PERF:MAIN] Editor Window: ready-to-show in ${Date.now() - perfStart}ms`);
		win.show();
	});

	win.webContents.on("did-finish-load", () => {
		console.log(`[PERF:MAIN] Editor Window: did-finish-load in ${Date.now() - perfStart}ms`);
		win?.webContents.send("main-process-message", new Date().toLocaleString());
		// Fallback for Linux/Wayland where `ready-to-show` may not fire reliably.
		if (!win.isDestroyed() && !win.isVisible()) {
			console.log("[editor-window] forcing show after did-finish-load");
			win.show();
		}
	});

	win.webContents.on("did-fail-load", (_event, errorCode, errorDescription, validatedURL) => {
		console.error("[editor-window] did-fail-load", {
			errorCode,
			errorDescription,
			validatedURL,
		});
	});

	win.webContents.on("render-process-gone", (_event, details) => {
		console.error("[editor-window] render-process-gone", details);
	});

	win.on("show", () => {
		console.log("[editor-window] show");
	});

	win.on("focus", () => {
		console.log("[editor-window] focus");
	});

	win.on("enter-full-screen", () => {
		if (!win.isDestroyed()) win.webContents.send("window-fullscreen-changed", true);
	});

	win.on("leave-full-screen", () => {
		if (!win.isDestroyed()) win.webContents.send("window-fullscreen-changed", false);
	});

	if (VITE_DEV_SERVER_URL) {
		const query = new URLSearchParams(getEditorWindowQuery());
		win.loadURL(`${VITE_DEV_SERVER_URL}?${query.toString()}`);
	} else {
		loadPackagedEditorWindow(win);
	}

	return win;
}

export function createSourceSelectorWindow(): BrowserWindow {
	const { width, height } = getScreen().getPrimaryDisplay().workAreaSize;

	const win = new BrowserWindow({
		width: 620,
		height: 420,
		minHeight: 350,
		maxHeight: 500,
		x: Math.round((width - 620) / 2),
		y: Math.round((height - 420) / 2),
		frame: false,
		resizable: false,
		alwaysOnTop: true,
		transparent: true,
		show: false,
		...(process.platform !== "darwin" && {
			icon: WINDOW_ICON_PATH,
		}),
		backgroundColor: "#00000000",
		webPreferences: {
			preload: path.join(electronWindowsDir, "preload.cjs"),
			nodeIntegration: false,
			contextIsolation: true,
		},
	});

	win.webContents.on("did-finish-load", () => {
		setTimeout(() => {
			if (!win.isDestroyed()) {
				win.show();
			}
		}, 100);
	});

	if (VITE_DEV_SERVER_URL) {
		win.loadURL(VITE_DEV_SERVER_URL + "?windowType=source-selector");
	} else {
		win.loadFile(path.join(RENDERER_DIST, "index.html"), {
			query: { windowType: "source-selector" },
		});
	}

	return win;
}

export function createCountdownWindow(): BrowserWindow {
	const primaryDisplay = getScreen().getPrimaryDisplay();
	const { x: workAreaX, y: workAreaY, width, height } = primaryDisplay.workArea;

	const windowSize = 200;
	const x = workAreaX + Math.floor((width - windowSize) / 2);
	const y = workAreaY + Math.floor((height - windowSize) / 2);

	const win = new BrowserWindow({
		width: windowSize,
		height: windowSize,
		x: x,
		y: y,
		frame: false,
		transparent: true,
		resizable: false,
		alwaysOnTop: true,
		skipTaskbar: true,
		hasShadow: false,
		focusable: true,
		show: false,
		webPreferences: {
			preload: path.join(electronWindowsDir, "preload.cjs"),
			nodeIntegration: false,
			contextIsolation: true,
		},
	});

	countdownWindow = win;

	win.setVisibleOnAllWorkspaces(true, {
		visibleOnFullScreen: true,
		// Keep DevCast a foreground application so macOS does not temporarily
		// remove its Dock icon while showing the countdown.
		skipTransformProcessType: process.platform === "darwin",
	});

	win.webContents.on("did-finish-load", () => {
		if (!win.isDestroyed()) {
			if (process.platform === "win32") {
				win.showInactive();
				win.moveTop();
			} else {
				win.show();
			}
		}
	});

	win.on("closed", () => {
		if (countdownWindow === win) {
			countdownWindow = null;
		}
	});

	if (VITE_DEV_SERVER_URL) {
		win.loadURL(VITE_DEV_SERVER_URL + "?windowType=countdown");
	} else {
		win.loadFile(path.join(RENDERER_DIST, "index.html"), {
			query: { windowType: "countdown" },
		});
	}

	return win;
}

export function getCountdownWindow(): BrowserWindow | null {
	return countdownWindow;
}

export function closeCountdownWindow(): void {
	if (countdownWindow && !countdownWindow.isDestroyed()) {
		countdownWindow.close();
		countdownWindow = null;
	}
}
