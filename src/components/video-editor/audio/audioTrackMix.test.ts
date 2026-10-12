import { describe, expect, it } from "vitest";
import { buildResolvedAudioPlan } from "@/lib/exporter/audioRoutingEngine";
import { applyAudioTrackMix } from "./audioTrackMix";

describe("user audio track mix", () => {
	it("applies one lane's volume and boost to all its sounds without changing other lanes", () => {
		const regions = [
			{ id: "a", startMs: 0, endMs: 1000, audioPath: "a.wav", volume: 0.8, gain: 1.5, trackIndex: 0 },
			{ id: "b", startMs: 1000, endMs: 2000, audioPath: "b.wav", volume: 1, gain: 1, trackIndex: 0 },
			{ id: "c", startMs: 0, endMs: 1000, audioPath: "c.wav", volume: 1, gain: 1, trackIndex: 1 },
		];
		const mixed = applyAudioTrackMix(regions, { 0: { volume: 0.5, gain: 2 } });
		const plan = buildResolvedAudioPlan({ videoResource: null, sourceAudioFallbackPaths: [], audioRegions: mixed });
		expect(plan.tracks[0].gain).toBeCloseTo(1.2);
		expect(plan.tracks.slice(1).map((track) => track.gain)).toEqual([1, 1]);
		expect(regions[0].volume).toBe(0.8);
	});
});
