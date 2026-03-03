import { HunkAssignmentRequest, HunkHeader, TreeChange } from "@gitbutler/but-sdk";
import { Match } from "effect";
import { type ChangeUnit } from "./ChangeUnit";
import { createDiffSpec } from "./DiffSpec";

export type RubSource = {
	parent: ChangeUnit;
	change: TreeChange;
	hunkHeaders: Array<HunkHeader>;
};

export type RubResult = {
	replacedCommits?: Record<string, string>;
	newCommit?: string | null;
	amendedCommitId?: string;
};

export const rub = async ({
	projectId,
	source,
	target,
}: {
	projectId: string;
	source: RubSource;
	target: ChangeUnit;
}): Promise<RubResult> => {
	const changes = [createDiffSpec(source.change, source.hunkHeaders)];

	return Match.value(source.parent).pipe(
		Match.tag("commit", (source) =>
			Match.value(target).pipe(
				Match.tag("commit", async (target): Promise<RubResult> => {
					const response = await window.lite.commitMoveChangesBetween({
						projectId,
						sourceCommitId: source.commitId,
						destinationCommitId: target.commitId,
						changes,
					});
					return { replacedCommits: response.replacedCommits };
				}),
				Match.tag("changes", async (target): Promise<RubResult> => {
					const response = await window.lite.commitUncommitChanges({
						projectId,
						commitId: source.commitId,
						assignTo: target.stackId,
						changes,
					});
					return {
						replacedCommits: response.replacedCommits,
					};
				}),
				Match.exhaustive,
			),
		),
		Match.tag("changes", () =>
			Match.value(target).pipe(
				Match.tag("commit", async (target): Promise<RubResult> => {
					const response = await window.lite.commitAmend({
						projectId,
						commitId: target.commitId,
						changes,
					});
					return {
						replacedCommits: response.replacedCommits,
						newCommit: response.newCommit ?? null,
						amendedCommitId: target.commitId,
					};
				}),
				Match.tag("changes", async (target): Promise<RubResult> => {
					await window.lite.assignHunk({
						projectId,
						assignments: source.hunkHeaders.map(
							(hunkHeader): HunkAssignmentRequest => ({
								pathBytes: source.change.pathBytes,
								hunkHeader,
								stackId: target.stackId,
							}),
						),
					});
					return {};
				}),
				Match.exhaustive,
			),
		),
		Match.exhaustive,
	);
};
