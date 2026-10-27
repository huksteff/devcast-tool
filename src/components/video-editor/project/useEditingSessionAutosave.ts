import { useCallback, useEffect, useLayoutEffect, useRef } from "react";
import { toast } from "@/components/ui/toast";
import type { EditorProjectData } from "../projectPersistence";

export function useEditingSessionAutosave(project: EditorProjectData | null, projectPath: string | null, loading: boolean) {
	const pending = useRef<{ project: EditorProjectData; path: string | null; key: string } | null>(null);
	const saved = useRef("");
	const flush = useCallback(() => {
		const current = pending.current;
		if (!current || saved.current === current.key || !window.electronAPI.saveEditingSession) return;
		try {
			const result = window.electronAPI.saveEditingSession(current.project, current.path);
			if (!result.success) throw new Error(result.error || "Could not save editing session");
			saved.current = current.key;
		} catch (error) {
			toast.error("Session autosave failed", { id: "editing-session-save", description: String(error), duration: Infinity });
		}
	}, []);
	useLayoutEffect(() => {
		if (loading || !project || !projectPath) { pending.current = null; return; }
		if (pending.current && (pending.current.project.videoPath !== project.videoPath || pending.current.path !== projectPath)) flush();
		pending.current = { project, path: projectPath, key: JSON.stringify([projectPath, project]) };
	}, [project, projectPath, loading, flush]);
	useEffect(() => {
		if (loading || !project || !projectPath) return;
		const timer = window.setTimeout(flush, 300);
		return () => window.clearTimeout(timer);
	}, [project, projectPath, loading, flush]);
	useEffect(() => {
		const periodicSave = window.setInterval(flush, 1000);
		// Synchronous IPC finishes the atomic disk write before Electron destroys
		// the renderer, including edits made within the debounce interval.
		window.addEventListener("beforeunload", flush);
		window.addEventListener("pagehide", flush);
		return () => {
			window.clearInterval(periodicSave);
			flush();
			window.removeEventListener("beforeunload", flush);
			window.removeEventListener("pagehide", flush);
		};
	}, [flush]);
	return flush;
}
