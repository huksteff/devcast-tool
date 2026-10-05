import { describe, expect, it } from "vitest";

import { getFinalMacCompanionAudioPath } from "./macCompanionAudio";

describe("mac companion audio paths", () => {
	it("preserves the helper's AAC container extension", () => {
		expect(
			getFinalMacCompanionAudioPath(
				"/Users/egg/DevCast/recording-1.mp4",
				"/Users/egg/DevCast/recording-1.mic.m4a",
				"mic",
			),
		).toBe("/Users/egg/DevCast/recording-1.mic.m4a");
	});

	it("preserves legacy sidecar extensions instead of renaming bytes", () => {
		expect(
			getFinalMacCompanionAudioPath(
				"/Users/egg/DevCast/recording-1.mp4",
				"/tmp/devcast-native.system.webm",
				"system",
			),
		).toBe("/Users/egg/DevCast/recording-1.system.webm");
	});

	it("keeps dotted directories when the video path has no extension", () => {
		expect(
			getFinalMacCompanionAudioPath(
				"/Users/egg/DevCast.videos/recording-1",
				"/tmp/devcast-native.mic.m4a",
				"mic",
			),
		).toBe("/Users/egg/DevCast.videos/recording-1.mic.m4a");
	});
});
