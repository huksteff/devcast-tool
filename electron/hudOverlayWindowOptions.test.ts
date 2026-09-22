import { describe, expect, it } from "vitest";
import { getHudOverlayTaskbarOptions } from "./hudOverlayWindowOptions";

describe("getHudOverlayTaskbarOptions", () => {
	it("does not activate the Windows HUD when its controls are clicked", () => {
		expect(getHudOverlayTaskbarOptions("win32")).toEqual({
			skipTaskbar: true,
			focusable: false,
		});
	});

	it.each([
		"darwin",
		"linux",
	] as const)("keeps the HUD non-focusable and out of the taskbar on %s", (platform) => {
		expect(getHudOverlayTaskbarOptions(platform)).toEqual({
			skipTaskbar: true,
			focusable: false,
		});
	});
});
