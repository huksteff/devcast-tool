import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { afterEach, beforeEach, expect, it } from "vitest";
import { createEditingSessionStore } from "./editingSessions";
import type { EditorProjectData } from "../../../src/components/video-editor/projectPersistence";

let root: string;
let video: string;
let named: string;
let store: ReturnType<typeof createEditingSessionStore>;
const project = (source: string, radius = 17): EditorProjectData => ({ version: 2, videoPath: source, editor: { borderRadius: radius, clipRegions: [], zoomRegions: [{ id: "zoom-1", startMs: 0, endMs: 1000, depth: 2, scale: 2.7, focus: { cx: 0.4, cy: 0.6 }, mode: "manual" }] } });
beforeEach(() => {
	root = fs.mkdtempSync(path.join(os.tmpdir(), "devcast-editing-session-"));
	video = path.join(root, "recording.mp4"); fs.writeFileSync(video, "video");
	named = path.join(root, "one.devcast"); fs.writeFileSync(named, "project");
	store = createEditingSessionStore(path.join(root, "sessions"));
});
afterEach(() => fs.rmSync(root, { recursive: true, force: true }));

it("does not save recovery state for a bare video", () => {
	store.save(project(video), null);
	expect(fs.existsSync(path.join(root, "sessions"))).toBe(false);
	expect(store.load(video)).toBeNull();
});

it("restores the full project session without changing the source video", () => {
	store.save(project(video), named);
	expect(store.load(video, named)).toEqual(project(video));
	expect(fs.readFileSync(video, "utf8")).toBe("video");
});
it("keeps projects separate even with the same video; a bare video never restores edits", () => {
	const second = path.join(root, "second.mp4"); fs.writeFileSync(second, "other");
	const named = path.join(root, "one.devcast"), copy = path.join(root, "two.devcast");
	fs.writeFileSync(named, "project"); fs.writeFileSync(copy, "project");
	store.save(project(video, 12), named);
	store.save(project(video, 27), copy);
	expect(store.load(video, named)?.editor.borderRadius).toBe(12);
	expect(store.load(video, copy)?.editor.borderRadius).toBe(27);
	expect(store.load(video)).toBeNull();
	expect(store.load(second)).toBeNull();
});
it("recovers the last complete generation after a truncated session file", () => {
	store.save(project(video, 12), named); store.save(project(video, 27), named);
	const file = fs.readdirSync(path.join(root, "sessions")).find(file => file.endsWith(".json"))!;
	fs.writeFileSync(path.join(root, "sessions", file), '{"version":');
	expect(store.load(video, named)?.editor.borderRadius).toBe(12);
});
it("does not apply an old edit when the video was replaced at the same path", () => {
	store.save(project(video), named); fs.writeFileSync(video, "different video");
	expect(store.load(video, named)).toBeNull();
});
it("prefers an externally updated project over its older recovery", () => {
	const named = path.join(root, "one.devcast"); fs.writeFileSync(named, "project");
	store.save(project(video), named);
	const future = new Date(Date.now() + 5000); fs.utimesSync(named, future, future);
	expect(store.load(video, named)).toBeNull();
});
it("retains newer session edits when an older queued project save finishes later", () => {
	const named = path.join(root, "one.devcast"); fs.writeFileSync(named, "old project");
	store.save(project(video, 12), named);
	store.save(project(video, 27), named);
	fs.writeFileSync(named, "queued older save");
	store.checkpoint(video, named);
	expect(store.load(video, named)?.editor.borderRadius).toBe(27);
});
