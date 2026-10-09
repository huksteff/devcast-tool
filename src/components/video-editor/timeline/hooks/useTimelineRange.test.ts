import { describe, expect, it } from "vitest";

import { preserveTimelineRange, resolveTimelineWheelPanDeltaPx } from "./useTimelineRange";

describe("resolveTimelineWheelPanDeltaPx", () => {
	it("uses trackpad horizontal wheel movement for timeline panning", () => {
		expect(
			resolveTimelineWheelPanDeltaPx({
				deltaX: 24,
				deltaY: 0,
				deltaMode: 0,
			}),
		).toBe(24);
	});

	it("uses shifted vertical wheel movement for timeline panning", () => {
		expect(
			resolveTimelineWheelPanDeltaPx({
				deltaX: 0,
				deltaY: 3,
				deltaMode: 1,
				shiftKey: true,
			}),
		).toBe(48);
	});

	it("keeps ctrl wheel available for timeline zoom unless shift is also held", () => {
		expect(
			resolveTimelineWheelPanDeltaPx({
				deltaX: 0,
				deltaY: 3,
				deltaMode: 1,
				ctrlKey: true,
			}),
		).toBe(0);
		expect(
			resolveTimelineWheelPanDeltaPx({
				deltaX: 0,
				deltaY: 3,
				deltaMode: 1,
				ctrlKey: true,
				shiftKey: true,
			}),
		).toBe(48);
	});

	it("uses regular wheel movement when the timeline has no vertical overflow", () => {
		expect(
			resolveTimelineWheelPanDeltaPx({
				deltaX: 0,
				deltaY: 20,
				deltaMode: 0,
				canScrollVertically: false,
			}),
		).toBe(20);
	});
});

describe("preserveTimelineRange", () => {
	it("initializes normally when video metadata arrives after an empty state", () => {
		expect(preserveTimelineRange({ start: 0, end: 0 }, 10_000)).toEqual({
			start: 0,
			end: 10_000,
		});
	});

	it("keeps the zoom and viewport after an item edit", () => {
		expect(preserveTimelineRange({ start: 2_000, end: 5_000 }, 10_000)).toEqual({
			start: 2_000,
			end: 5_000,
		});
	});

	it("clamps the viewport without expanding it when duration shrinks", () => {
		expect(preserveTimelineRange({ start: 8_000, end: 10_000 }, 9_000)).toEqual({
			start: 7_000,
			end: 9_000,
		});
	});
});
