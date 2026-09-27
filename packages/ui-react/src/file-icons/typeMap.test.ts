import { readdirSync } from "node:fs";
import { describe, expect, test } from "vitest";
import { symbolFileExtensionsToIcons, symbolFileNamesToIcons } from "./typeMap.ts";

const icons = new Set(
	readdirSync(new URL("svg", import.meta.url)).map((file) => file.replace(/\.svg$/, "")),
);

describe.each([
	["extensions", symbolFileExtensionsToIcons],
	["file names", symbolFileNamesToIcons],
])("%s", (_, map) => {
	// A name with no SVG behind it falls through to the generic document icon without a word.
	test("map to icons that exist", () => {
		expect(Object.values(map).filter((icon) => !icons.has(icon))).toEqual([]);
	});

	// FileIcon lowercases the file name before looking it up, so a key with a capital never matches.
	test("are lowercase", () => {
		expect(Object.keys(map).filter((key) => key !== key.toLowerCase())).toEqual([]);
	});
});
