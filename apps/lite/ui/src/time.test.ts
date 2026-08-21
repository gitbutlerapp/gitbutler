import { describe, expect, it, vi } from "vitest";

// Node v22 is missing this API, causing the test suite to throw on importing the module.
vi.hoisted(() => {
	Object.defineProperty(Intl, "DurationFormat", {
		value: class DurationFormat {},
	});
});

import { formatCompactDurationWith, formatRelativeTimeWith } from "./time.ts";

describe("formatRelativeTime", () => {
	const now = 1_800_000_000_000;
	const formatRelativeTime = formatRelativeTimeWith(
		new Intl.RelativeTimeFormat("en", { numeric: "always", style: "long" }),
	);

	it("formats seconds", () => {
		expect(formatRelativeTime(now - 2_000, now)).toMatchInlineSnapshot(`"2 seconds ago"`);
	});

	it("formats minutes", () => {
		expect(formatRelativeTime(now - 2 * 60_000, now)).toMatchInlineSnapshot(`"2 minutes ago"`);
	});

	it("formats hours", () => {
		expect(formatRelativeTime(now - 2 * 60 * 60_000, now)).toMatchInlineSnapshot(`"2 hours ago"`);
	});

	it("formats days", () => {
		expect(formatRelativeTime(now - 2 * 24 * 60 * 60_000, now)).toMatchInlineSnapshot(
			`"2 days ago"`,
		);
	});

	it("formats months", () => {
		expect(formatRelativeTime(now - 2 * 30 * 24 * 60 * 60_000, now)).toMatchInlineSnapshot(
			`"2 months ago"`,
		);
	});

	it("formats years", () => {
		expect(formatRelativeTime(now - 2 * 365 * 24 * 60 * 60_000, now)).toMatchInlineSnapshot(
			`"2 years ago"`,
		);
	});
});

describe("formatCompactDuration", () => {
	// Node lacks Intl.DurationFormat (see the stub above), so assert on the
	// unit the formatter picks rather than on localised output.
	const formatCompactDuration = formatCompactDurationWith({
		format: (duration) => JSON.stringify(duration),
	} as Intl.DurationFormat);

	it("rounds a sub-second duration up to a second", () => {
		expect(formatCompactDuration(120)).toMatchInlineSnapshot(`"{"seconds":1}"`);
	});

	it("formats seconds", () => {
		expect(formatCompactDuration(45_000)).toMatchInlineSnapshot(`"{"seconds":45}"`);
	});

	it("formats minutes", () => {
		expect(formatCompactDuration(12 * 60_000)).toMatchInlineSnapshot(`"{"minutes":12}"`);
	});

	it("formats hours", () => {
		expect(formatCompactDuration(2 * 60 * 60_000)).toMatchInlineSnapshot(`"{"hours":2}"`);
	});

	it("carries a rounded-up second into the next unit", () => {
		expect(formatCompactDuration(59_600)).toMatchInlineSnapshot(`"{"minutes":1}"`);
	});

	it("carries a rounded-up minute into the next unit", () => {
		expect(formatCompactDuration(59 * 60_000 + 59_000)).toMatchInlineSnapshot(`"{"hours":1}"`);
	});
});
