import {
	buildPrDescriptionPrompt,
	prDescriptionGenerationButtonState,
	splitGeneratedDescription,
} from "#ui/pr-description-generation.ts";
import type { Commit } from "@gitbutler/but-sdk";
import { describe, expect, test } from "vitest";

const commit = (message: string) => ({ message }) as Commit;

describe("prDescriptionGenerationButtonState", () => {
	const base = { enabled: true, configured: true, busy: false, commitCount: 1 };

	test("reports an unconfigured provider ahead of a disabled project setting", () => {
		expect(
			prDescriptionGenerationButtonState({ ...base, configured: false, enabled: false }),
		).toEqual({ disabled: true, hint: "Set up AI in Settings → Application → AI" });
	});

	test("asks for the project setting once a provider exists", () => {
		expect(prDescriptionGenerationButtonState({ ...base, enabled: false })).toEqual({
			disabled: true,
			hint: "Enable AI in Settings → Project → AI",
		});
	});

	test("says nothing while the branch's commits are still loading", () => {
		expect(prDescriptionGenerationButtonState({ ...base, commitCount: undefined })).toEqual({
			disabled: true,
			hint: null,
		});
	});

	test("has nothing to describe without commits", () => {
		expect(prDescriptionGenerationButtonState({ ...base, commitCount: 0 })).toEqual({
			disabled: true,
			hint: "No commits to describe",
		});
	});

	test("locks while busy, but keeps the plain label", () => {
		expect(prDescriptionGenerationButtonState({ ...base, busy: true })).toEqual({
			disabled: true,
			hint: null,
		});
	});

	test("is ready when set up and idle", () => {
		expect(prDescriptionGenerationButtonState(base)).toEqual({ disabled: false, hint: null });
	});
});

describe("buildPrDescriptionPrompt", () => {
	test("orders commits oldest first, as the work happened", () => {
		const prompt = buildPrDescriptionPrompt("", "", [commit("newest"), commit("oldest")]);
		expect(prompt.indexOf("oldest")).toBeLessThan(prompt.indexOf("newest"));
	});

	test("omits empty title and body rather than sending blank sections", () => {
		const prompt = buildPrDescriptionPrompt("  ", "", [commit("only commit")]);
		expect(prompt).not.toContain("Working title");
		expect(prompt).not.toContain("Description so far");
	});

	test("passes a partly written title and body as context", () => {
		const prompt = buildPrDescriptionPrompt("Add uploads", "Still drafting", [commit("c")]);
		expect(prompt).toContain("Add uploads");
		expect(prompt).toContain("Still drafting");
	});
});

describe("splitGeneratedDescription", () => {
	test("takes the first line as the title and the rest as the body", () => {
		expect(splitGeneratedDescription("Add file uploads\n\n- one\n- two")).toEqual({
			title: "Add file uploads",
			body: "- one\n- two",
		});
	});

	test("drops a body heading that just restates the title", () => {
		expect(splitGeneratedDescription("Add file uploads\n\n# Add file uploads\n\n- one")).toEqual({
			title: "Add file uploads",
			body: "- one",
		});
	});

	test("keeps a body heading that says something else", () => {
		expect(splitGeneratedDescription("Add file uploads\n\n## Background\n\n- one")).toEqual({
			title: "Add file uploads",
			body: "## Background\n\n- one",
		});
	});

	test("strips markup the model wrapped the title in anyway", () => {
		expect(splitGeneratedDescription('# "Add file uploads"\n\nbody').title).toBe(
			"Add file uploads",
		);
	});

	test("has no body yet when only the title has streamed in", () => {
		expect(splitGeneratedDescription("Add file up")).toEqual({
			title: "Add file up",
			body: "",
		});
	});
});

describe("splitGeneratedDescription: fenced answers", () => {
	test("unwraps a whole answer wrapped in a fence", () => {
		expect(splitGeneratedDescription("```markdown\nAdd uploads\n\n- one\n```")).toEqual({
			title: "Add uploads",
			body: "- one",
		});
	});

	test("keeps a fenced code block that is only part of the body", () => {
		const body = "Run it:\n\n```sh\nnpm test\n```\n\nThen check the output.";
		expect(splitGeneratedDescription(`Add uploads\n\n${body}`).body).toBe(body);
	});

	test("drops a lone opener mid-stream, before the closing fence arrives", () => {
		expect(splitGeneratedDescription("```markdown\nAdd file up")).toEqual({
			title: "Add file up",
			body: "",
		});
	});
});
