import { HunkAssignmentRequest, HunkHeader, TreeChange } from "@gitbutler/but-sdk";
import { Match } from "effect";
import { type ChangeUnit } from "#ui/ChangeUnit.ts";
import { createDiffSpec } from "#ui/DiffSpec.ts";

export type FilePatchRubSource = {
	parent: ChangeUnit;
	change: TreeChange;
	hunkHeaders: Array<HunkHeader>;
};

export type RubSource = { _tag: "FilePatch"; source: FilePatchRubSource };

/** @public */
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
}): Promise<RubResult> =>
	Match.value(source).pipe(
		Match.tag("FilePatch", ({ source }) => {
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
		}),
		Match.exhaustive,
	);

type RubOperation = "Amend" | "Uncommit" | "Assign" | "Unassign";

export const rubOperationFor = (rubSource: RubSource, target: ChangeUnit): RubOperation | null =>
	Match.value(rubSource).pipe(
		Match.tag("FilePatch", ({ source }) =>
			Match.value(source.parent).pipe(
				Match.tag("commit", (source) =>
					Match.value(target).pipe(
						Match.tag("commit", (target): RubOperation | null => {
							if (source.commitId === target.commitId) return null;
							return "Amend";
						}),
						Match.tag("changes", (): RubOperation => "Uncommit"),
						Match.exhaustive,
					),
				),
				Match.tag("changes", (source) =>
					Match.value(target).pipe(
						Match.tag("commit", (): RubOperation => "Amend"),
						Match.tag("changes", (target): RubOperation | null => {
							if (source.stackId === target.stackId) return null;
							return target.stackId === null ? "Unassign" : "Assign";
						}),
						Match.exhaustive,
					),
				),
				Match.exhaustive,
			),
		),
		Match.exhaustive,
	);
