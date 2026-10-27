import { type Dispatch, type SetStateAction, useCallback, useEffect, useRef, useState } from "react";
import { toast } from "@/components/ui/toast";
import { resolveAutoCaptionSourcePath } from "../autoCaptionSource";
import { type CaptionEditTarget, updateCaptionCuesForEditedTarget } from "../captionEditing";
import { resolveVideoUrl } from "../projectPersistence";
import type { AutoCaptionSettings, CaptionCue } from "../types";
import { getErrorMessage } from "../videoEditorUtils";

type DownloadStatus = "idle" | "downloading" | "downloaded" | "error";
type Translator = (
	key: string,
	fallback?: string,
	params?: Record<string, string | number>,
) => string;

interface UseAutoCaptionControllerParams {
	t: Translator;
	videoPath: string | null;
	setVideoPath: Dispatch<SetStateAction<string | null>>;
	videoSourcePath: string | null;
	setVideoSourcePath: Dispatch<SetStateAction<string | null>>;
	webcamSourcePath: string | null;
	whisperExecutablePath: string | null;
	setWhisperExecutablePath: Dispatch<SetStateAction<string | null>>;
	whisperModelPath: string | null;
	setWhisperModelPath: Dispatch<SetStateAction<string | null>>;
	downloadedWhisperModelPath: string | null;
	setDownloadedWhisperModelPath: Dispatch<SetStateAction<string | null>>;
	whisperModelDownloadStatus: DownloadStatus;
	setWhisperModelDownloadStatus: Dispatch<SetStateAction<DownloadStatus>>;
	setWhisperModelDownloadProgress: Dispatch<SetStateAction<number>>;
	isGeneratingCaptions: boolean;
	setIsGeneratingCaptions: Dispatch<SetStateAction<boolean>>;
	autoCaptionSettings: AutoCaptionSettings;
	setAutoCaptionSettings: Dispatch<SetStateAction<AutoCaptionSettings>>;
	setAutoCaptions: Dispatch<SetStateAction<CaptionCue[]>>;
	syncActiveVideoSource: (sourcePath: string, webcamPath?: string | null) => Promise<void>;
}

export function useAutoCaptionController({
	t,
	videoPath,
	setVideoPath,
	videoSourcePath,
	setVideoSourcePath,
	webcamSourcePath,
	whisperExecutablePath,
	setWhisperExecutablePath,
	whisperModelPath,
	setWhisperModelPath,
	downloadedWhisperModelPath,
	setDownloadedWhisperModelPath,
	whisperModelDownloadStatus,
	setWhisperModelDownloadStatus,
	setWhisperModelDownloadProgress,
	isGeneratingCaptions,
	setIsGeneratingCaptions,
	autoCaptionSettings,
	setAutoCaptionSettings,
	setAutoCaptions,
	syncActiveVideoSource,
}: UseAutoCaptionControllerParams) {
	const captionGenerationInFlightRef = useRef(false);
	const modelStatusVersionRef = useRef(0);
	const [captionGenerationProgress, setCaptionGenerationProgress] = useState(0);
	const activeSourceRef = useRef({ videoSourcePath, videoPath });
	activeSourceRef.current = { videoSourcePath, videoPath };

	useEffect(() => {
		const unsubscribe = window.electronAPI.onWhisperSmallModelDownloadProgress((state) => {
			modelStatusVersionRef.current++;
			setWhisperModelDownloadStatus(state.status);
			setWhisperModelDownloadProgress(state.progress);
			if (state.status === "downloaded") {
				setDownloadedWhisperModelPath(state.path ?? null);
				setWhisperModelPath((current) => current ?? state.path ?? null);
			} else if (state.status === "idle") {
				setDownloadedWhisperModelPath(null);
				setWhisperModelPath((current) => current === state.path ? null : current);
			} else if (state.status === "error" && state.error) {
				toast.error(state.error);
			}
		});

		const statusVersion = modelStatusVersionRef.current;
		void window.electronAPI.getWhisperSmallModelStatus().then((result) => {
			if (modelStatusVersionRef.current !== statusVersion) return;
			if (!result.success) return;
			if (result.exists && result.path) {
				setDownloadedWhisperModelPath(result.path);
				setWhisperModelPath((current) => current ?? result.path ?? null);
				setWhisperModelDownloadStatus("downloaded");
				setWhisperModelDownloadProgress(100);
			} else {
				setDownloadedWhisperModelPath(null);
				setWhisperModelPath((current) => current === result.path ? null : current);
				setWhisperModelDownloadStatus("idle");
				setWhisperModelDownloadProgress(0);
			}
		});

		return () => unsubscribe?.();
	}, [
		setDownloadedWhisperModelPath,
		setWhisperModelDownloadProgress,
		setWhisperModelDownloadStatus,
		setWhisperModelPath,
	]);

	const handlePickWhisperExecutable = useCallback(async () => {
		const result = await window.electronAPI.openWhisperExecutablePicker();
		if (!result.success || !result.path) return;
		setWhisperExecutablePath(result.path);
		toast.success("Whisper executable selected");
	}, [setWhisperExecutablePath]);

	const handleDownloadWhisperSmallModel = useCallback(async () => {
		if (whisperModelDownloadStatus === "downloading") return;
		modelStatusVersionRef.current++;
		setWhisperModelDownloadStatus("downloading");
		setWhisperModelDownloadProgress(0);
		const result = await window.electronAPI.downloadWhisperSmallModel();
		if (!result.success) {
			setWhisperModelDownloadStatus("error");
			toast.error(result.error || "Failed to download Whisper small model");
			return;
		}
		if (result.path) {
			setDownloadedWhisperModelPath(result.path);
			setWhisperModelPath(result.path);
			setWhisperModelDownloadStatus("downloaded");
			setWhisperModelDownloadProgress(100);
		}
	}, [
		setDownloadedWhisperModelPath,
		setWhisperModelDownloadProgress,
		setWhisperModelDownloadStatus,
		setWhisperModelPath,
		whisperModelDownloadStatus,
	]);

	const handlePickWhisperModel = useCallback(async () => {
		const result = await window.electronAPI.openWhisperModelPicker();
		if (!result.success || !result.path) return;
		setWhisperModelPath(result.path);
		toast.success("Whisper model selected");
	}, [setWhisperModelPath]);

	const handleDeleteWhisperSmallModel = useCallback(async () => {
		modelStatusVersionRef.current++;
		const result = await window.electronAPI.deleteWhisperSmallModel();
		if (!result.success) {
			toast.error(result.error || "Failed to delete Whisper small model");
			return;
		}
		setWhisperModelPath((current) =>
			current === downloadedWhisperModelPath || current === result.path ? null : current);
		setDownloadedWhisperModelPath(null);
		setWhisperModelDownloadStatus("idle");
		setWhisperModelDownloadProgress(0);
		toast.success("Whisper small model deleted");
	}, [
		downloadedWhisperModelPath,
		setDownloadedWhisperModelPath,
		setWhisperModelDownloadProgress,
		setWhisperModelDownloadStatus,
		setWhisperModelPath,
	]);

	const handleGenerateAutoCaptions = useCallback(async () => {
		if (captionGenerationInFlightRef.current || isGeneratingCaptions) return;
		captionGenerationInFlightRef.current = true;
		setIsGeneratingCaptions(true);
		setCaptionGenerationProgress(6);
		const progressTimer = setInterval(() => {
			setCaptionGenerationProgress((current) => Math.min(88, current + Math.max(1, Math.round((90 - current) / 7))));
		}, 900);
		try {
			if (!whisperModelPath) {
				toast.error("Select a Whisper model or download the small model first");
				return;
			}
			let sourcePath = resolveAutoCaptionSourcePath({ videoSourcePath, videoPath });
			if (!sourcePath) {
				const sessionResult = await window.electronAPI.getCurrentRecordingSession?.();
				const currentVideoResult = await window.electronAPI.getCurrentVideoPath();
				sourcePath = resolveAutoCaptionSourcePath({
					recordingSessionVideoPath:
						sessionResult?.success && sessionResult.session?.videoPath
							? sessionResult.session.videoPath
							: null,
					currentVideoPath: currentVideoResult.success
						? (currentVideoResult.path ?? null)
						: null,
				});
			}
			if (!sourcePath) {
				toast.error("No source video is loaded");
				return;
			}
			// Refreshing an unchanged session revokes active companion-audio URLs.
			if (sourcePath !== videoSourcePath) {
				await syncActiveVideoSource(sourcePath, webcamSourcePath);
				setVideoSourcePath(sourcePath);
				setVideoPath(await resolveVideoUrl(sourcePath));
			}
			const result = await window.electronAPI.generateAutoCaptions({
				videoPath: sourcePath,
				whisperExecutablePath: whisperExecutablePath ?? undefined,
				whisperModelPath,
				language: autoCaptionSettings.language,
			});
			if (resolveAutoCaptionSourcePath(activeSourceRef.current) !== sourcePath) {
				toast.info("Recording changed; generated captions were not applied.");
				return;
			}
			if (!result.success || !result.cues) {
				const errorMessage = result.error ? getErrorMessage(result.error) : result.message;
				if (/ENOENT.*(?:ggml|\.bin)|Whisper model.*(?:missing|not found)/i.test(errorMessage || "")) {
					setWhisperModelPath(null);
					setDownloadedWhisperModelPath(null);
					setWhisperModelDownloadStatus("idle");
					setWhisperModelDownloadProgress(0);
				}
				toast.error(errorMessage || "Failed to generate captions");
				return;
			}
			setAutoCaptions(result.cues);
			setCaptionGenerationProgress(100);
			if (result.cues.length > 0) {
				setAutoCaptionSettings((current) => ({ ...current, enabled: true }));
			}
			toast.success(result.message || `Generated ${result.cues.length} captions`);
		} catch (error) {
			toast.error(getErrorMessage(error));
		} finally {
			clearInterval(progressTimer);
			captionGenerationInFlightRef.current = false;
			setIsGeneratingCaptions(false);
			setTimeout(() => setCaptionGenerationProgress(0), 500);
		}
	}, [
		autoCaptionSettings.language,
		isGeneratingCaptions,
		setAutoCaptionSettings,
		setAutoCaptions,
		setWhisperModelPath,
		setDownloadedWhisperModelPath,
		setWhisperModelDownloadStatus,
		setWhisperModelDownloadProgress,
		setIsGeneratingCaptions,
		setVideoPath,
		setVideoSourcePath,
		syncActiveVideoSource,
		videoPath,
		videoSourcePath,
		webcamSourcePath,
		whisperExecutablePath,
		whisperModelPath,
	]);

	const handleSaveAutoCaptionEdit = useCallback(
		(target: CaptionEditTarget, text: string) => {
			setAutoCaptions((captions) => updateCaptionCuesForEditedTarget(captions, target, text));
			toast.success(t("settings.captions.editSaved", "Caption updated"));
		},
		[setAutoCaptions, t],
	);

	return {
		handlePickWhisperExecutable,
		handleDownloadWhisperSmallModel,
		handlePickWhisperModel,
		handleDeleteWhisperSmallModel,
		handleGenerateAutoCaptions,
		captionGenerationProgress,
		handleSaveAutoCaptionEdit,
	};
}
