import { afterEach, describe, expect, it, vi } from "vitest";
import { useAudioPreviewSync } from "./useAudioPreviewSync";

const harness = vi.hoisted(() => ({
	effects: [] as (() => void | (() => void))[],
	refs: [] as { current: unknown }[],
	index: 0,
	loaded: vi.fn(),
}));
vi.mock("react", () => ({
	useCallback: (callback: unknown) => callback,
	useMemo: (factory: () => unknown) => factory(),
	useEffect: (effect: () => void) => harness.effects.push(effect),
	useRef: (value: unknown) => {
		const index = harness.index++;
		harness.refs[index] ??= { current: value };
		return harness.refs[index];
	},
	useState: () => [0, harness.loaded],
}));
vi.mock("@/lib/exporter/localMediaSource", () => ({
	resolveMediaElementSource: async () => ({ src: "file:///audio.wav", revoke: vi.fn() }),
}));
vi.mock("../videoPlayback/playbackRate", () => ({
	supportsPreviewPlaybackRate: (rate: number) => rate <= 16,
}));

afterEach(() => {
	vi.unstubAllGlobals();
	harness.effects = [];
	harness.refs = [];
	harness.index = 0;
	harness.loaded.mockClear();
});

describe("source preview playback ownership", () => {
	it("applies boost above 100% through a gain node and can mute it", () => {
		const audio = {
			src: "",
			dataset: {},
			volume: 1,
			paused: true,
			preload: "",
			crossOrigin: "",
			pause: vi.fn(),
			load: vi.fn(),
			play: vi.fn().mockResolvedValue(undefined),
		};
		const gains: Array<{ gain: { value: number }; connect: ReturnType<typeof vi.fn> }> = [];
		vi.stubGlobal("Audio", function () {
			return audio;
		});
		vi.stubGlobal(
			"AudioContext",
			class {
				state = "running";
				destination = {};
				createGain() {
					const node = { gain: { value: 1 }, connect: vi.fn() };
					gains.push(node);
					return node;
				}
				createMediaElementSource() {
					return { connect: vi.fn() };
				}
			},
		);
		const render = (boost: number) => {
			harness.index = 0;
			harness.effects = [];
			useAudioPreviewSync({
				audioRegions: [],
				previewVolume: 1,
				isPlaying: false,
				currentTime: 0,
				timelineTime: 0,
				duration: 10,
				sourcePlaybackRate: 1,
				previewSourceAudioFallbackPaths: ["/audio.system.wav"],
				sourceAudioFallbackStartDelayMsByPath: {},
				sourceAudioResourceVersion: 0,
				isCurrentClipMuted: false,
				getSourceTrackPreviewGain: () => boost,
				onSourceFallbackLoadError: vi.fn(),
			});
			for (const effect of harness.effects) effect();
		};
		render(10);
		expect(gains[1]?.gain.value).toBe(10);
		render(0);
		expect(gains[1]?.gain.value).toBe(0);
	});
	it.each([
		{
			name: "sub-second offset",
			muted: false,
			playing: true,
			rate: 1,
			time: 0.5,
			delay: 0,
			plays: true,
		},
		{
			name: "playing clip",
			muted: false,
			playing: true,
			rate: 1,
			time: 1,
			delay: 0,
			plays: true,
		},
		{
			name: "gap or muted clip",
			muted: true,
			playing: true,
			rate: 1,
			time: 1,
			delay: 0,
			plays: false,
		},
		{
			name: "paused clip",
			muted: false,
			playing: false,
			rate: 1,
			time: 1,
			delay: 0,
			plays: false,
		},
		{
			name: "unsupported rate",
			muted: false,
			playing: true,
			rate: 20,
			time: 1,
			delay: 0,
			plays: false,
		},
		{
			name: "before audio start",
			muted: false,
			playing: true,
			rate: 1,
			time: 0,
			delay: 1000,
			plays: false,
		},
		{
			name: "media end",
			muted: false,
			playing: true,
			rate: 1,
			time: 10,
			delay: 0,
			plays: false,
		},
	])("checks $name after a late source load", async ({
		muted,
		playing,
		rate,
		time,
		delay,
		plays,
	}) => {
		const audio = {
			src: "",
			dataset: {},
			duration: 10,
			currentTime: 0,
			playbackRate: 1,
			paused: true,
			volume: 1,
			load: vi.fn(),
			pause: vi.fn(),
			play: vi.fn().mockResolvedValue(undefined),
		};
		vi.stubGlobal("Audio", function () {
			return audio;
		});
		vi.stubGlobal(
			"AudioContext",
			class {
				state = "running";
				destination = {};
				createGain() {
					return { gain: { value: 1 }, connect: vi.fn() };
				}
			},
		);
		// Execute mocked effects explicitly so the asynchronous load can finish between syncs.
		useAudioPreviewSync({
			audioRegions: [],
			previewVolume: 1,
			isPlaying: playing,
			currentTime: time,
			timelineTime: time,
			duration: 10,
			sourcePlaybackRate: rate,
			previewSourceAudioFallbackPaths: ["/audio.system.wav"],
			sourceAudioFallbackStartDelayMsByPath: { "/audio.system.wav": delay },
			sourceAudioResourceVersion: 0,
			isCurrentClipMuted: muted,
			getSourceTrackPreviewGain: () => 1,
			onSourceFallbackLoadError: vi.fn(),
		});
		for (const effect of harness.effects) effect();
		await Promise.resolve();
		expect(harness.loaded).toHaveBeenCalledOnce();
		expect(audio.play).not.toHaveBeenCalled();
		harness.effects.at(-1)?.();
		await Promise.resolve();
		expect(audio.play).toHaveBeenCalledTimes(plays ? 1 : 0);
		if (plays) expect(audio.currentTime).toBeCloseTo(time - delay / 1000);
	});
});
