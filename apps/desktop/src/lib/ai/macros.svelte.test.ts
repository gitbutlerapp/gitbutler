import DiffInputContext from "$lib/ai/diffInputContext.svelte";
import AIMacros from "$lib/ai/macros.svelte";
import { showToast } from "$lib/notifications/toasts";
import { beforeEach, expect, test, vi } from "vitest";
import type { PromptService } from "$lib/ai/aiPromptService";
import type { AIService } from "$lib/ai/service";
import type { ChangeDiff, DiffService } from "$lib/hunks/diffService.svelte";
import type { UncommittedService } from "$lib/selection/uncommittedService.svelte";
import type { StackService } from "$lib/stacks/stackService.svelte";
import type { WorktreeService } from "$lib/worktree/worktreeService.svelte";

vi.mock("$lib/notifications/toasts", () => ({ showToast: vi.fn() }));

const binaryDiffs: ChangeDiff[] = [
	{ path: "first.pdf", diff: { type: "Binary" } },
	{ path: "second.pdf", diff: { type: "Binary" } },
];
const textDiff: ChangeDiff = {
	path: "file.txt",
	diff: {
		type: "Patch",
		subject: {
			isResultOfBinaryToTextConversion: false,
			linesAdded: 1,
			linesRemoved: 1,
			hunks: [
				{
					diff: "@@ -1 +1 @@\n-old\n+new\n",
					oldStart: 1,
					oldLines: 1,
					newStart: 1,
					newLines: 1,
				},
			],
		},
	},
};

async function setup(diffs: ChangeDiff[]) {
	const aiService = {
		validateConfiguration: vi.fn().mockResolvedValue(true),
		summarizeCommit: vi.fn().mockImplementation(async ({ onToken }) => {
			onToken?.("Generated message");
			return "Generated message";
		}),
		summarizeBranch: vi.fn().mockResolvedValue("generated-branch"),
	};
	const context = new DiffInputContext(
		{} as WorktreeService,
		{ fetchChanges: vi.fn().mockResolvedValue(diffs) } as unknown as DiffService,
		{} as StackService,
		{
			type: "change-selection",
			projectId: "project-id",
			uncommittedService: {
				selectedChanges: vi.fn().mockReturnValue(diffs.map(({ path }) => ({ path }))),
				filterDiffsBasedOnSelection: (selectedDiffs: ChangeDiff[]) => selectedDiffs,
			} as unknown as UncommittedService,
		},
	);
	const macros = new AIMacros(
		"project-id",
		aiService as unknown as AIService,
		{
			selectedCommitPrompt: vi.fn().mockReturnValue("Commit prompt"),
			selectedBranchPrompt: vi.fn().mockReturnValue("Branch prompt"),
		} as unknown as PromptService,
		context,
	);
	await macros.setGenAIEnabled(true);
	return { macros, aiService };
}

beforeEach(() => vi.clearAllMocks());

test.each([
	["binary-only selection", binaryDiffs],
	["no selection", []],
] as const)("skips generation for %s without streaming a replacement", async (_, diffs) => {
	const { macros, aiService } = await setup([...diffs]);
	const onToken = vi.fn();

	expect(await macros.generateCommitMessage({ onToken })).toBeUndefined();
	expect(aiService.summarizeCommit).not.toHaveBeenCalled();
	expect(onToken).not.toHaveBeenCalled();
	expect(showToast).toHaveBeenCalledExactlyOnceWith({
		style: "info",
		message: "Nothing to summarize yet — add or select some changes first.",
	});
});

test.each([
	["text-only selection", [textDiff]],
	["mixed text and binary selection", [...binaryDiffs, textDiff]],
] as const)("generates from supported text in %s", async (_, diffs) => {
	const { macros, aiService } = await setup([...diffs]);
	const onToken = vi.fn();

	expect(await macros.generateCommitMessage({ onToken })).toBe("Generated message");
	expect(aiService.summarizeCommit).toHaveBeenCalledExactlyOnceWith(
		expect.objectContaining({
			diffInput: [{ filePath: "file.txt", diff: "@@ -1 +1 @@\n-old\n+new\n" }],
		}),
	);
	expect(onToken).toHaveBeenCalledExactlyOnceWith("Generated message");
	expect(showToast).not.toHaveBeenCalled();
});

test("skips an explicitly empty diff input", async () => {
	const { macros, aiService } = await setup([textDiff]);
	expect(await macros.generateCommitMessage({ diffInput: [] })).toBeUndefined();
	expect(aiService.summarizeCommit).not.toHaveBeenCalled();
});

test("skips combined branch and commit generation for binary-only selection", async () => {
	const { macros, aiService } = await setup(binaryDiffs);
	expect(await macros.getBranchNameAndCommitMessage()).toEqual({
		branchName: undefined,
		commitMessage: undefined,
	});
	expect(aiService.summarizeBranch).not.toHaveBeenCalled();
	expect(aiService.summarizeCommit).not.toHaveBeenCalled();
	expect(showToast).toHaveBeenCalledTimes(1);
});
