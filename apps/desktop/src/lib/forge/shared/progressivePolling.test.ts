import { getPollingInterval } from "$lib/forge/shared/progressivePolling";
import { describe, expect, test } from "vitest";

const SECOND = 1000;
const MINUTE = 60 * SECOND;

const INITIAL = 5 * SECOND;
const SHORT = 30 * SECOND;
const MEDIUM = 5 * MINUTE;
const LONG = 30 * MINUTE;

describe("getPollingInterval", () => {
	test("shouldStop halts polling regardless of errors", () => {
		expect(getPollingInterval(0, true, 0)).toBe(0);
		expect(getPollingInterval(0, true, 5)).toBe(0);
	});

	test("healthy query follows the progressive schedule", () => {
		expect(getPollingInterval(0, false, 0)).toBe(INITIAL);
		expect(getPollingInterval(5 * MINUTE, false, 0)).toBe(SHORT);
		expect(getPollingInterval(30 * MINUTE, false, 0)).toBe(MEDIUM);
		expect(getPollingInterval(2 * 60 * MINUTE, false, 0)).toBe(LONG);
	});

	test("first failed poll steps out to the short interval", () => {
		// Fast (initial) schedule + one error → short, not a hard jump to medium.
		expect(getPollingInterval(0, false, 1)).toBe(SHORT);
	});

	test("sustained failures escalate to the medium interval", () => {
		expect(getPollingInterval(0, false, 2)).toBe(MEDIUM);
		expect(getPollingInterval(0, false, 5)).toBe(MEDIUM);
	});

	test("backoff never polls faster than the normal schedule", () => {
		// Already on the long schedule: an error must not speed polling up.
		expect(getPollingInterval(2 * 60 * MINUTE, false, 1)).toBe(LONG);
		expect(getPollingInterval(2 * 60 * MINUTE, false, 2)).toBe(LONG);
	});
});
