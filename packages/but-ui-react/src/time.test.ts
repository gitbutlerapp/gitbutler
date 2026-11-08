import { describe, expect, it } from "vitest";

import { formatCompactDurationWith, formatRelativeTimeWith } from "./time.ts";

describe("formatRelativeTime", () => {
	const now = 1_800_000_000_000;
	const formatRelativeTime = formatRelativeTimeWith(
		new Intl.RelativeTimeFormat("en", { numeric: "always", style: "long" }),
	);

	it("uses just now for the first minute", () => {
		expect(formatRelativeTime(now, now)).toBe("just now");
		expect(formatRelativeTime(now - 2_000, now)).toBe("just now");
		expect(formatRelativeTime(now - 59_999, now)).toBe("just now");
		expect(formatRelativeTime(now - 60_000, now)).toBe("1 minute ago");
		expect(formatRelativeTime(now + 2_000, now)).toBe("in 2 seconds");
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
	const formatCompactDuration = formatCompactDurationWith(
		new Intl.DurationFormat("en", { style: "short" }),
	);

	it("rounds a sub-second duration up to a second", () => {
		expect(formatCompactDuration(120)).toBe("1 sec");
	});

	it("formats seconds", () => {
		expect(formatCompactDuration(45_000)).toBe("45 sec");
	});

	it("formats minutes", () => {
		expect(formatCompactDuration(12 * 60_000)).toBe("12 min");
	});

	it("formats hours", () => {
		expect(formatCompactDuration(2 * 60 * 60_000)).toBe("2 hr");
	});

	it("carries a rounded-up second into the next unit", () => {
		expect(formatCompactDuration(59_600)).toBe("1 min");
	});

	it("carries a rounded-up minute into the next unit", () => {
		expect(formatCompactDuration(59 * 60_000 + 59_000)).toBe("1 hr");
	});
});
