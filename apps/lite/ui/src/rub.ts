import { HunkAssignmentRequest, HunkHeader, TreeChange } from "@gitbutler/but-sdk";
import { Match } from "effect";
import { type ChangeUnit } from "#ui/ChangeUnit.ts";
import { createDiffSpec } from "#ui/DiffSpec.ts";

export type FilePatchRubSource = {
	parent: ChangeUnit;
	change: TreeChange;
	hunkHeaders: Array<HunkHeader>;
};

export type CommitRubSource = {
	commitId: string;
};

export type RubSource =
	| { _tag: "FilePatch"; source: FilePatchRubSource }
	| { _tag: "Commit"; source: CommitRubSource };

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
						Match.withReturnType<Promise<RubResult>>(),
						Match.tag("commit", async (target) => {
							const response = await window.lite.commitMoveChangesBetween({
								projectId,
								sourceCommitId: source.commitId,
								destinationCommitId: target.commitId,
								changes,
							});
							return { replacedCommits: response.replacedCommits };
						}),
						Match.tag("changes", async (target) => {
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
						Match.withReturnType<Promise<RubResult>>(),
						Match.tag("commit", async (target) => {
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
						Match.tag("changes", async (target) => {
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
		// TODO: implement squashing when API is ready
		Match.tag("Commit", async (): Promise<RubResult> => ({})),
		Match.exhaustive,
	);

type RubOperation = "Amend" | "Uncommit" | "Assign" | "Unassign" | "Squash";

export const rubOperationFor = (rubSource: RubSource, target: ChangeUnit): RubOperation | null =>
	Match.value(rubSource).pipe(
		Match.withReturnType<RubOperation | null>(),
		Match.tag("FilePatch", ({ source }) =>
			Match.value(source.parent).pipe(
				Match.withReturnType<RubOperation | null>(),
				Match.tag("commit", (source) =>
					Match.value(target).pipe(
						Match.withReturnType<RubOperation | null>(),
						Match.tag("commit", (target) => {
							if (source.commitId === target.commitId) return null;
							return "Amend";
						}),
						Match.tag("changes", () => "Uncommit"),
						Match.exhaustive,
					),
				),
				Match.tag("changes", (source) =>
					Match.value(target).pipe(
						Match.withReturnType<RubOperation | null>(),
						Match.tag("commit", () => "Amend"),
						Match.tag("changes", (target) => {
							if (source.stackId === target.stackId) return null;
							return target.stackId === null ? "Unassign" : "Assign";
						}),
						Match.exhaustive,
					),
				),
				Match.exhaustive,
			),
		),
		Match.tag("Commit", (source) =>
			Match.value(target).pipe(
				Match.withReturnType<RubOperation | null>(),
				Match.tag("commit", (target) => {
					if (source.source.commitId === target.commitId) return null;
					return "Squash";
				}),
				Match.tag("changes", () => null),
				Match.exhaustive,
			),
		),
		Match.exhaustive,
	);
