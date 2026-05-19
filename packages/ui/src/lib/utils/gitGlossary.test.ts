import { splitGlossaryText } from "$lib/utils/gitGlossary";
import { describe, expect, test } from "vitest";

function matchedTerms(text: string, terms?: Parameters<typeof splitGlossaryText>[1]) {
	return splitGlossaryText(text, terms)
		.filter((segment) => segment.type === "term")
		.map((segment) => ({ term: segment.term, text: segment.text }));
}

describe.concurrent("splitGlossaryText", () => {
	test("matches requested git terms in plain copy", () => {
		expect(
			matchedTerms("Rebase upstream changes before you push.", ["rebase", "upstream", "push"]),
		).toEqual([
			{ term: "rebase", text: "Rebase" },
			{ term: "upstream", text: "upstream" },
			{ term: "push", text: "push" },
		]);
	});

	test("prefers longer aliases like merge conflict over merge", () => {
		expect(
			matchedTerms("Resolve the merge conflict before you merge.", ["merge", "merge-conflict"]),
		).toEqual([
			{ term: "merge-conflict", text: "merge conflict" },
			{ term: "merge", text: "merge" },
		]);
	});

	test("matches punctuation-adjacent aliases like --rebase", () => {
		expect(
			matchedTerms("This is similar to git pull --rebase after a fetch.", [
				"pull",
				"rebase",
				"fetch",
			]),
		).toEqual([
			{ term: "pull", text: "pull" },
			{ term: "rebase", text: "rebase" },
			{ term: "fetch", text: "fetch" },
		]);
	});
});
