import { plainErrorMessage } from "../../../electron/src/plain-error";
import { describe, expect, test } from "vitest";

describe("plain error messages", () => {
	test("hides Git terminology for conflicts", () => {
		expect(plainErrorMessage(new Error("merge conflict"))).toBe(
			"This project has work in a shape How cannot save automatically yet.",
		);
	});

	test("falls back to a simple save failure", () => {
		expect(plainErrorMessage(new Error("strange low-level error"))).toBe(
			"How could not save this checkpoint.",
		);
	});
});
