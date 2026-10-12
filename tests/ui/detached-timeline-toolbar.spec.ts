import { expect, test } from "@playwright/test";
import type { TimelineWindowSnapshot } from "../../src/components/video-editor/timeline/timelineWindowProtocol";

test.use({ channel: process.env.DEVCAST_UI_BROWSER_CHANNEL === "chrome" ? "chrome" : undefined });

test("detached timeline shows keyframes and runs its editing toolbar", async ({ page }) => {
	const snapshot = {
		isPlaying: false,
		keyframeMode: "auto",
		showKeyframes: true,
		selectedKeyframeId: null,
		canGenerateAutoCaptions: true,
		isGeneratingCaptions: false,
		rowScale: 1,
		videoDuration: 10,
		currentTime: 2,
		playheadTime: 2,
		videoPath: null,
		videoSourcePath: null,
		cursorTelemetrySourcePath: null,
		cursorTelemetry: [],
		autoSuggestZoomsTrigger: 0,
		disableSuggestedZooms: false,
		zoomRegions: [],
		selectedZoomId: null,
		trimRegions: [],
		clipRegions: [{ id: "clip-1", startMs: 0, endMs: 10000, sourceStartMs: 0, speed: 1 }],
		selectedClipId: null,
		audioRegions: [],
		audioTrackCount: 1,
		selectedAudioId: null,
		captionRegions: [],
		selectedCaptionId: null,
		captionsEnabled: false,
		captionQuickAddEnabled: false,
		annotationRegions: [
			{
				id: "annotation-1",
				startMs: 1000,
				endMs: 7000,
				type: "text",
				content: "Example",
				position: { x: 0, y: 0 },
				size: { width: 20, height: 10 },
				zIndex: 0,
				trackIndex: 0,
				positionKeyframes: [
					{
						id: "keyframe-at-playhead",
						timeMs: 2000,
						position: { x: 1, y: 0 },
						size: { width: 20, height: 10 },
					},
					{
						id: "keyframe-1",
						timeMs: 3000,
						position: { x: 0, y: 0 },
						size: { width: 20, height: 10 },
					},
				],
			},
		],
		selectedAnnotationId: null,
		showSourceAudioTrack: false,
		sourceAudioResourceVersion: 0,
		sourceAudioTrackSettings: {},
	} as unknown as TimelineWindowSnapshot;
	await page.addInitScript((initialSnapshot) => {
		const state = {
			snapshot: initialSnapshot,
			listener: null as ((message: unknown) => void) | null,
			actions: [] as unknown[],
		};
		Object.assign(window, {
			__timelineTest: state,
			electronAPI: {
				onTimelineWindowMessage: (listener: (message: unknown) => void) => {
					state.listener = listener;
					return () => {
						state.listener = null;
					};
				},
				sendTimelineWindowMessage: (message: { type: string; action?: unknown }) => {
					if (message.type === "snapshot-request")
						queueMicrotask(() =>
							state.listener?.({ type: "snapshot", snapshot: state.snapshot }),
						);
					if (message.type === "action") state.actions.push(message.action);
				},
			},
		});
	}, snapshot);
	await page.goto("/?windowType=timeline-window");
	await expect(page.getByRole("toolbar", { name: "Timeline tools" })).toBeVisible();
	await expect(page.getByTitle("Keyframe @ 2000ms (drag to move)")).toBeVisible();
	await expect(page.getByTitle("Keyframe @ 3000ms (drag to move)")).toBeVisible();
	await page.getByTitle("Keyframe @ 2000ms (drag to move)").hover();
	await expect(page.getByText("Position.x: 0 → 1")).toBeVisible();
	await expect(page.locator('[data-timeline-row="row-audio-0"]')).toBeVisible();
	await expect(page.getByRole("button", { name: "+ Add sound" })).toBeVisible();
	await page.getByTitle("Keyframe @ 2000ms (drag to move)").click();
	await page.evaluate(() => {
		const state = (window as unknown as { __timelineTest: { snapshot: TimelineWindowSnapshot; listener: ((message: unknown) => void) | null } }).__timelineTest;
		state.snapshot = { ...state.snapshot, selectedKeyframeId: "keyframe-at-playhead" };
		state.listener?.({ type: "snapshot", snapshot: state.snapshot });
	});
	await page.getByRole("button", { name: "Copy selected keyframe" }).click();
	await page.getByRole("button", { name: "Paste keyframe at playhead" }).click();
	await expect(page.getByRole("button", { name: "Disable automatic keyframes" })).toHaveAttribute(
		"aria-pressed",
		"true",
	);
	await page.getByRole("button", { name: "Disable automatic keyframes" }).click();
	await page.getByRole("button", { name: "Keyframe editing mode" }).click();
	await page.getByRole("menuitem", { name: "Manual keyframes (I)" }).click();
	await page.getByRole("button", { name: "Hide keyframes" }).click();
	await page.getByRole("button", { name: "Generate Captions" }).click();
	await page.getByRole("button", { name: /Add Zoom/ }).click();
	await page.getByRole("button", { name: "Add Layer" }).click();
	await page.getByRole("menuitem", { name: "Annotation" }).click();
	await page.getByRole("button", { name: "Add Layer" }).click();
	await page.getByRole("menuitem", { name: "Add audio track" }).click();
	await page.evaluate(() => {
		const state = (window as unknown as {
			__timelineTest: {
				snapshot: TimelineWindowSnapshot;
				listener: ((message: unknown) => void) | null;
			};
		}).__timelineTest;
		state.snapshot = { ...state.snapshot, showKeyframes: false };
		state.listener?.({ type: "snapshot", snapshot: state.snapshot });
	});
	await expect(page.getByTitle("Keyframe @ 2000ms (drag to move)")).toHaveCount(0);
	const actions = await page.evaluate(
		() =>
			(window as unknown as { __timelineTest: { actions: Array<{ type: string }> } })
				.__timelineTest.actions,
	);
	const actionTypes = actions.map((action) => action.type);
	expect(actionTypes).toContain("select-annotation");
	expect(actionTypes).toContain("select-keyframe");
	expect(actionTypes).toContain("copy-keyframe");
	expect(actionTypes).toContain("paste-keyframe");
	expect(actionTypes).toContain("audio-track-added");
	await expect(actionTypes.slice(-7, -1)).toEqual([
		"toggle-keyframe-mode",
		"set-keyframe-mode",
		"toggle-keyframe-visibility",
		"generate-captions",
		"zoom-added",
		"annotation-added",
	]);
});
