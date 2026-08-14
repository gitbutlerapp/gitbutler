import { describe, expect, it, vi } from "vitest";
import type { TreeChange, UnifiedPatch } from "@gitbutler/but-sdk";
import {
	buildCommitMessagePrompt,
	changesSelectedForCommit,
	commitMessageGenerationButtonState,
	streamCommitMessage,
} from "./commit-message-generation.ts";

const change = (path: string): TreeChange =>
	({ path, status: { type: "Modification" } }) as TreeChange;

describe("commit message generation", () => {
	it("shows only configured project AI and disables the button while busy", () => {
		expect(
			commitMessageGenerationButtonState({
				enabled: true,
				configured: true,
				busy: false,
				changeCount: 1,
			}),
		).toEqual({ visible: true, disabled: false });
		expect(
			commitMessageGenerationButtonState({
				enabled: false,
				configured: true,
				busy: true,
				changeCount: 1,
			}),
		).toEqual({ visible: false, disabled: true });
	});

	it("uses all changes when nothing is checked and otherwise filters by path", () => {
		const changes = [change("one.ts"), change("two.ts")];
		expect(changesSelectedForCommit(changes, new Set())).toEqual(changes);
		expect(changesSelectedForCommit(changes, new Set(["two.ts"]))).toEqual([changes[1]]);
	});

	it("formats patch markers and caps the appended diff", () => {
		const patch = {
			type: "Patch",
			subject: { hunks: [{ diff: `@@ -1 +1 @@\n-${"a".repeat(6_000)}` }] },
		} as UnifiedPatch;
		const prompt = buildCommitMessagePrompt("Instructions", [change("one.ts")], [patch]);
		const diff = prompt.match(/```diff\n([\s\S]*)\n```$/)?.[1];

		expect(prompt).toContain("File: one.ts (Modification)");
		expect(diff).toHaveLength(5_000);
	});

	it("preserves the current value on a failed stream", async () => {
		const onValue = vi.fn();
		await expect(
			streamCommitMessage(
				async () => {
					throw new Error("failed before response");
				},
				onValue,
				"original",
			),
		).rejects.toThrow("failed before response");
		expect(onValue).not.toHaveBeenCalled();

		await expect(
			streamCommitMessage(
				async (onToken) => {
					onToken("partial");
					throw new Error("failed during response");
				},
				onValue,
				"original",
			),
		).rejects.toThrow("failed during response");
		expect(onValue.mock.calls).toEqual([["partial"], ["original"]]);

		onValue.mockClear();
		await streamCommitMessage(
			async (onToken) => {
				onToken("feat: ");
				onToken("generated");
				return "feat: generated";
			},
			onValue,
			"original",
		);
		expect(onValue.mock.calls).toEqual([["feat: "], ["feat: generated"], ["feat: generated"]]);
	});
});
