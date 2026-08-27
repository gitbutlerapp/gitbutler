import { describe, expect, it } from "vitest";

import {
	ageBadgeOpacity,
	formatAgeBadgeWith,
	formatCompactDurationWith,
	formatRelativeTimeWith,
} from "./time.ts";

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

describe("formatAgeBadge", () => {
	const formatAgeBadge = formatAgeBadgeWith(new Intl.DurationFormat("en", { style: "narrow" }));

	it("calls anything under a minute now", () => {
		expect(formatAgeBadge(0)).toBe("now");
		expect(formatAgeBadge(59_000)).toBe("now");
	});

	it("abbreviates minutes", () => {
		expect(formatAgeBadge(3 * 60_000)).toBe("3m");
		expect(formatAgeBadge(59 * 60_000)).toBe("59m");
	});

	it("abbreviates hours", () => {
		expect(formatAgeBadge(60 * 60_000)).toBe("1h");
		expect(formatAgeBadge(2.5 * 60 * 60_000)).toBe("2h");
	});

	it("abbreviates days and weeks", () => {
		expect(formatAgeBadge(24 * 60 * 60_000)).toBe("1d");
		expect(formatAgeBadge(6 * 24 * 60 * 60_000)).toBe("6d");
		expect(formatAgeBadge(7 * 24 * 60 * 60_000)).toBe("1w");
	});

	it("labels every age, however old", () => {
		expect(formatAgeBadge(3 * 60 * 60_000 + 1)).toBe("3h");
		expect(formatAgeBadge(365 * 24 * 60 * 60_000)).toBe("52w");
	});
});

describe("ageBadgeOpacity", () => {
	it("gives a just-written change full contrast", () => {
		expect(ageBadgeOpacity(0)).toBe(1);
	});

	it("fades monotonically as the change recedes", () => {
		const ages = [0, 60_000, 60 * 60_000, 24 * 60 * 60_000].map(ageBadgeOpacity);
		expect(ages).toEqual([...ages].sort((a, b) => b - a));
		expect(new Set(ages).size).toBe(ages.length);
	});

	it("floors instead of fading to nothing", () => {
		expect(ageBadgeOpacity(24 * 60 * 60_000)).toBeCloseTo(0.35, 5);
		// A year old is no fainter than a day old, so the badge stays readable.
		expect(ageBadgeOpacity(365 * 24 * 60 * 60_000)).toBeCloseTo(0.35, 5);
	});
});
