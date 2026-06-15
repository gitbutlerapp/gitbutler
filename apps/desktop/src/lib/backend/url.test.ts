import { getEditorUri } from "$lib/backend/url";
import { describe, expect, test } from "vitest";

describe("getEditorUri", () => {
	test.each([
		"vscode",
		"vscode-insiders",
		"vscodium",
		"cursor",
		"windsurf",
		"trae",
		"antigravity-ide",
	])("%s keeps query parameters", (schemeId) => {
		expect(
			getEditorUri({
				schemeId,
				path: ["/home/example/project"],
				searchParams: { windowId: "_blank" },
			}),
		).toBe(`${schemeId}://file/home/example/project?windowId=_blank`);
	});

	test("zed drops query parameters since it treats them as part of the file path", () => {
		expect(
			getEditorUri({
				schemeId: "zed",
				path: ["/home/example/project"],
				searchParams: { windowId: "_blank" },
			}),
		).toBe("zed://file/home/example/project");
	});

	test("line and column suffixes are appended for all editors", () => {
		expect(
			getEditorUri({
				schemeId: "zed",
				path: ["/home/example/project", "src/main.rs"],
				line: 42,
				column: 7,
			}),
		).toBe("zed://file/home/example/project/src/main.rs:42:7");
	});

	test("uris without search params are unchanged", () => {
		expect(getEditorUri({ schemeId: "vscode", path: ["/home/example/project"] })).toBe(
			"vscode://file/home/example/project",
		);
	});
});
