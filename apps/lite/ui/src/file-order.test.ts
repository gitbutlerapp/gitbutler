import { describe, expect, test } from "vitest";
import { compareFilePaths } from "./file-order.ts";

describe("file tree ordering", () => {
	test("orders path items as a directory-first tree reveals them", () => {
		const unsorted = [
			"readme.md",
			"src/ui/v10.ts",
			"src/app.ts",
			"src/ui/Outline.tsx",
			"docs/guide.md",
			"src/ui/v9.ts",
			"src/ui/lineStats.ts",
		];

		expect(unsorted.toSorted(compareFilePaths)).toMatchInlineSnapshot(`
			[
			  "docs/guide.md",
			  "src/ui/lineStats.ts",
			  "src/ui/Outline.tsx",
			  "src/ui/v9.ts",
			  "src/ui/v10.ts",
			  "src/app.ts",
			  "readme.md",
			]
		`);
	});
});
