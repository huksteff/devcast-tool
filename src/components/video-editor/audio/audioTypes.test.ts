import { describe, expect, it } from "vitest";
import { resolveRecordingTrackGain } from "./audioTypes";

describe("recording audio gain", () => {
	it("applies the main Audio boost to a separate system track", () => {
		const settings = {
			mixed: { volume: 1, gain: 10, normalize: false },
			system: { volume: 1, gain: 1, normalize: false },
		};
		expect(resolveRecordingTrackGain(settings, "system")).toBe(10);
		expect(resolveRecordingTrackGain(settings, "mixed")).toBe(10);
		settings.mixed.gain = 0;
		expect(resolveRecordingTrackGain(settings, "system")).toBe(0);
	});
});
