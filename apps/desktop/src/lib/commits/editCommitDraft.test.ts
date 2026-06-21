import { applyEditDraftUpdate, resolveEditDraft } from "$lib/commits/editCommitDraft";
import { describe, expect, test } from "vitest";

const original = { title: "Original title", description: "Original body" };

describe("resolveEditDraft", () => {
	test("uses the original message when there is no draft", () => {
		expect(resolveEditDraft(undefined, "c1", original)).toEqual(original);
	});

	test("uses the original message when the draft is for another commit", () => {
		const draft = { commitId: "c2", title: "Other", description: "Other body" };
		expect(resolveEditDraft(draft, "c1", original)).toEqual(original);
	});

	test("uses the draft when it belongs to the commit", () => {
		const draft = { commitId: "c1", title: "Draft title", description: "Draft body" };
		expect(resolveEditDraft(draft, "c1", original)).toEqual({
			title: "Draft title",
			description: "Draft body",
		});
	});
});

describe("applyEditDraftUpdate", () => {
	test("seeds from the original message on the first change", () => {
		expect(applyEditDraftUpdate(undefined, "c1", original, { title: "New title" })).toEqual({
			commitId: "c1",
			title: "New title",
			description: "Original body",
		});
	});

	test("merges a description-only change, keeping the existing title", () => {
		const draft = { commitId: "c1", title: "A", description: "B" };
		expect(applyEditDraftUpdate(draft, "c1", original, { description: "B2" })).toEqual({
			commitId: "c1",
			title: "A",
			description: "B2",
		});
	});

	test("restarts from the original when the draft is for another commit", () => {
		const draft = { commitId: "c2", title: "A", description: "B" };
		expect(applyEditDraftUpdate(draft, "c1", original, { title: "New" })).toEqual({
			commitId: "c1",
			title: "New",
			description: "Original body",
		});
	});

	test("keeps an explicitly cleared (empty) field", () => {
		const draft = { commitId: "c1", title: "A", description: "B" };
		expect(applyEditDraftUpdate(draft, "c1", original, { title: "" })).toEqual({
			commitId: "c1",
			title: "",
			description: "B",
		});
	});
});
