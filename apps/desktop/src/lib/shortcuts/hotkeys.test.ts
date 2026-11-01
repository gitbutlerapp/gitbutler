import { isSelectAllChord } from "$lib/shortcuts/hotkeys";
import { describe, expect, test } from "vitest";

function keydown(key: string, modifiers: { metaKey?: boolean; ctrlKey?: boolean } = {}) {
	return new KeyboardEvent("keydown", { key, ...modifiers });
}

/** The values `Backend["platformName"]` takes where ⌘ is not the select-all modifier. */
const NON_MAC_PLATFORMS = ["windows", "linux", "web"] as const;

describe("isSelectAllChord", () => {
	test("macOS selects all with cmd", () => {
		expect(isSelectAllChord(keydown("a", { metaKey: true }), "macos")).toBe(true);
	});

	// Ctrl+A is the emacs "move to the start of the line" binding that macOS text fields have,
	// so claiming it for select-all takes an editing key away instead of adding a shortcut.
	test("macOS leaves ctrl alone", () => {
		expect(isSelectAllChord(keydown("a", { ctrlKey: true }), "macos")).toBe(false);
	});

	test.each(NON_MAC_PLATFORMS)("%s selects all with ctrl", (platform) => {
		expect(isSelectAllChord(keydown("a", { ctrlKey: true }), platform)).toBe(true);
	});

	// Cmd is not a select-all modifier off macOS, where that key is the Super/Windows key.
	test.each(NON_MAC_PLATFORMS)("%s ignores meta", (platform) => {
		expect(isSelectAllChord(keydown("a", { metaKey: true }), platform)).toBe(false);
	});

	test("a bare 'a' types rather than selects", () => {
		expect(isSelectAllChord(keydown("a"), "macos")).toBe(false);
		expect(isSelectAllChord(keydown("a"), "windows")).toBe(false);
	});

	test("the modifier only counts together with 'a'", () => {
		expect(isSelectAllChord(keydown("b", { metaKey: true }), "macos")).toBe(false);
		expect(isSelectAllChord(keydown("b", { ctrlKey: true }), "windows")).toBe(false);
	});
});
