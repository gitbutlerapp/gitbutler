import { highlightDependencyCommitRows } from "$lib/dependencies/dependencyHighlights";
import { afterEach, describe, expect, test } from "vitest";

afterEach(() => document.body.replaceChildren());

describe("dependency commit highlighting", () => {
	test("keeps rows highlighted until every interaction owner clears", () => {
		document.body.innerHTML = `
			<div data-commit-id="first"></div>
			<div data-commit-id="shared"></div>
			<div data-commit-id="second"></div>
		`;

		const clearFirst = highlightDependencyCommitRows(["first", "shared", "absent"]);
		const clearSecond = highlightDependencyCommitRows(["shared", "second"]);

		expect(document.querySelectorAll(".dependency-highlighted")).toHaveLength(3);
		clearFirst();
		expect(document.querySelector('[data-commit-id="first"]')).not.toHaveClass(
			"dependency-highlighted",
		);
		expect(document.querySelector('[data-commit-id="shared"]')).toHaveClass(
			"dependency-highlighted",
		);
		clearSecond();
		expect(document.querySelectorAll(".dependency-highlighted")).toHaveLength(0);
	});
});
