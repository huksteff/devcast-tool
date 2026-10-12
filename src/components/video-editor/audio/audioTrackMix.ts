import type { AudioRegion } from "../types";

export interface AudioTrackMixSetting {
	volume: number;
	gain: number;
}

export type AudioTrackMix = Record<number, AudioTrackMixSetting>;

export function applyAudioTrackMix(regions: AudioRegion[], mix: AudioTrackMix): AudioRegion[] {
	return regions.map((region) => {
		const setting = mix[region.trackIndex ?? 0];
		if (!setting) return region;
		return {
			...region,
			volume: region.volume * setting.volume,
			gain: (region.gain ?? 1) * setting.gain,
		};
	});
}
