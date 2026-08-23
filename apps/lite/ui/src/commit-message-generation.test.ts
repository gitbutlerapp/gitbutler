import { describe, expect, it } from "vitest";
import type { TreeChange, UnifiedPatch } from "@gitbutler/but-sdk";
import {
	buildCommitMessagePrompt,
	changesSelectedForCommit,
	commitMessageGenerationButtonState,
} from "./commit-message-generation.ts";

const change = (path: string): TreeChange =>
	({ path, status: { type: "Modification" } }) as TreeChange;

describe("commit message generation", () => {
	it("hints at what blocks generation and disables the button while busy", () => {
		expect(
			commitMessageGenerationButtonState({
				enabled: true,
				configured: true,
				busy: false,
				changeCount: 1,
			}),
		).toEqual({ disabled: false, hint: null });
		expect(
			commitMessageGenerationButtonState({
				enabled: true,
				configured: true,
				busy: true,
				changeCount: 1,
			}),
		).toEqual({ disabled: true, hint: null });
		// An unconfigured provider is reported over a disabled project setting,
		// since the setting can't be turned on without one.
		expect(
			commitMessageGenerationButtonState({
				enabled: false,
				configured: false,
				busy: false,
				changeCount: 1,
			}).hint,
		).toContain("Application → AI");
		expect(
			commitMessageGenerationButtonState({
				enabled: false,
				configured: true,
				busy: false,
				changeCount: 1,
			}).hint,
		).toContain("Project → AI");
		expect(
			commitMessageGenerationButtonState({
				enabled: true,
				configured: true,
				busy: false,
				changeCount: 0,
			}),
		).toEqual({ disabled: true, hint: "No changes to commit" });
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
});
