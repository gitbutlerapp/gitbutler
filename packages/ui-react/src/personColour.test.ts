import { describe, expect, test } from "vitest";
import { personColour, personInitial } from "./personColour.ts";

describe("personColour", () => {
	test("gives the same colour for the same seed", () => {
		expect(personColour("krlvi")).toEqual(personColour("krlvi"));
	});

	// The GPUI client ports this; its `person.rs` checks the same seeds.
	test("picks what the GPUI client picks", () => {
		expect(personColour("Ada Agent")).toBe("var(--scale-danger-70)");
		expect(personColour("Kiril Videlov")).toBe("var(--scale-safe-70)");
		expect(personColour("é😀")).toBe("var(--scale-pop-70)");
		expect(personColour("😀")).toBe("var(--scale-safe-70)");
	});
});

describe("personInitial", () => {
	test("takes the first letter or digit, upper-cased", () => {
		expect(personInitial("krlvi")).toBe("K");
		expect(personInitial("pavel@gitbutler.com")).toBe("P");
		expect(personInitial("-estib")).toBe("E");
		expect(personInitial("42bot")).toBe("4");
		expect(personInitial("é😀")).toBe("É");
	});

	test("is empty without a letter or digit", () => {
		expect(personInitial("")).toBe("");
		expect(personInitial("😀")).toBe("");
	});
});
