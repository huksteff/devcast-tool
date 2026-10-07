import type { AudioPeaksData } from "../timeline/core/timelineTypes";

export type SourceAudioTrackId = "mixed" | "system" | "mic" | (string & {});

export interface SourceAudioTrackSetting {
	/** Base fader: 0–100%. */
	volume: number;
	/** Post-fader amplification, up to 10× for intentionally loud sources. */
	gain?: number;
	normalize: boolean;
}

export type SourceAudioTrackSettings = Record<string, SourceAudioTrackSetting>;

export interface SourceAudioTrackMetaItem {
	id: SourceAudioTrackId;
	label: string;
}

export type SourceAudioTrackMeta = SourceAudioTrackMetaItem[];

export interface SourceAudioTrackWithPeaks extends SourceAudioTrackMetaItem {
	peaks: AudioPeaksData;
}

export const SOURCE_AUDIO_FALLBACK_TOAST_ID = "source-audio-fallback-error";
export const SOURCE_AUDIO_NORMALIZE_GAIN = 1.35;

export function resolveRecordingTrackGain(
	settingsByTrack: SourceAudioTrackSettings | undefined,
	trackId: "mixed" | "system" | "mic",
): number {
	const track = settingsByTrack?.[trackId];
	const master = trackId === "mixed" ? undefined : settingsByTrack?.mixed;
	const level = (setting: SourceAudioTrackSetting | undefined) =>
		Math.max(0, Math.min(1, setting?.volume ?? 1)) *
		Math.max(0, Math.min(10, setting?.gain ?? 1)) *
		(setting?.normalize ? SOURCE_AUDIO_NORMALIZE_GAIN : 1);
	return Math.max(0, Math.min(10, level(track) * level(master)));
}
