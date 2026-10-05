import { describe, expect, test } from "vitest";
import { commitBody, commitTitle } from "./commit.ts";

describe("commit title and body", () => {
	test("split a message at its first line", () => {
		expect(commitTitle("Add search\n\nWith a filter.")).toBe("Add search");
		expect(commitBody("Add search\n\nWith a filter.")).toBe("With a filter.");
	});

	test("name a published snapshot by its branch instead of showing its JSON", () => {
		const message = JSON.stringify({
			version: 1,
			title: "todo-app",
			head: "refs/heads/agent/search",
			target: "refs/remotes/origin/main",
			machine: "studio",
		});
		expect(commitTitle(message)).toBe("Uncommitted changes on agent/search");
		expect(commitBody(message)).toBeUndefined();
	});

	test("leave messages that only look like JSON alone", () => {
		expect(commitTitle("{ not json")).toBe("{ not json");
	});
});
