import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import type { EditorProjectData } from "../../../src/components/video-editor/projectPersistence";

function identity(file: string) {
	const stat = fs.statSync(file);
	return { size: stat.size, modified: stat.mtimeMs };
}
function key(file: string) {
	const resolved = path.resolve(file);
	return process.platform === "win32" ? resolved.toLowerCase() : resolved;
}

/** Separate recovery files; never overwrite the user's project or presets. */
export function createEditingSessionStore(directory: string) {
	const sessionFile = (video: string, projectPath?: string | null) => path.join(directory,
		`${createHash("sha256").update(`${key(video)}\0${projectPath ? key(projectPath) : "video"}`).digest("hex")}.json`);
	return {
		checkpoint(video: string, projectPath: string) {
			// A queued normal project save may finish after a newer recovery write.
			// Update its file identity without replacing the newer editing state.
			const target = sessionFile(video, projectPath);
			for (const file of [target, `${target}.bak`]) {
				if (!fs.existsSync(file)) continue;
				const saved = JSON.parse(fs.readFileSync(file, "utf8"));
				saved.projectFile = identity(projectPath);
				const temp = `${file}.tmp`;
				fs.writeFileSync(temp, JSON.stringify(saved), "utf8");
				fs.renameSync(temp, file);
			}
		},
		save(project: EditorProjectData, projectPath?: string | null) {
			if (!projectPath) return;
			const projectFile = projectPath && fs.existsSync(projectPath) ? identity(projectPath) : null;
			const payload = JSON.stringify({ version: 1, savedAt: Date.now(), media: identity(project.videoPath), projectFile, project });
			fs.mkdirSync(directory, { recursive: true });
			const targets = [sessionFile(project.videoPath, projectPath)];
			for (const target of targets) {
				const temp = `${target}.tmp`;
				const handle = fs.openSync(temp, "w");
				try { fs.writeFileSync(handle, payload, "utf8"); fs.fsyncSync(handle); }
				finally { fs.closeSync(handle); }
				if (fs.existsSync(target)) fs.copyFileSync(target, `${target}.bak`);
				fs.renameSync(temp, target);
			}
		},
		load(video: string, projectPath?: string | null): EditorProjectData | null {
			if (!projectPath) return null;
			const target = sessionFile(video, projectPath);
			const media = identity(video);
			for (const file of [target, `${target}.bak`]) {
				try {
					const saved = JSON.parse(fs.readFileSync(file, "utf8"));
					if (saved.version !== 1 || !saved.project?.editor || typeof saved.project.videoPath !== "string") continue;
					if (key(saved.project.videoPath) !== key(video) || saved.media?.size !== media.size || saved.media?.modified !== media.modified) continue;
					// An explicitly saved external project takes precedence over an older recovery.
					if (projectPath) {
						const currentProject = identity(projectPath);
						if (currentProject.size !== saved.projectFile?.size || currentProject.modified !== saved.projectFile?.modified) return null;
					}
					return saved.project;
				} catch (error) {
					if ((error as NodeJS.ErrnoException).code && (error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
				}
			}
			return null;
		},
	};
}
