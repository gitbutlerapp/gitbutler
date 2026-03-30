import {
	HunkAssignmentRequest,
	HunkHeader,
	TreeChange,
	UICommitCreateResult,
} from "@gitbutler/but-sdk";
import { Match } from "effect";
import { type ChangeUnit } from "#ui/domain/ChangeUnit.ts";
import { createDiffSpec } from "#ui/domain/DiffSpec.ts";

export type TreeChangesRubSource = {
	parent: ChangeUnit;
	changes: Array<{
		change: TreeChange;
		hunkHeaders: Array<HunkHeader>;
	}>;
};

export type CommitRubSource = {
	commitId: string;
};

export type RubSource =
	| { _tag: "TreeChanges"; source: TreeChangesRubSource }
	| { _tag: "Commit"; source: CommitRubSource };

export type RubParams = {
	projectId: string;
	source: RubSource;
	target: ChangeUnit;
};

/** @public */
export type RubResult = {
	replacedCommits?: Record<string, string>;
	newCommit?: string | null;
	amendedCommitId?: string;
	rejectedChanges?: UICommitCreateResult["rejectedChanges"];
};

// In the future this may be implemented as a single API endpoint on the backend.
export const rub = async ({ projectId, source, target }: RubParams): Promise<RubResult> =>
	Match.value(source).pipe(
		Match.tag("TreeChanges", ({ source }) =>
			Match.value(source.parent).pipe(
				Match.tag("Changes", () =>
					Match.value(target).pipe(
						Match.tag("Changes", async (target): Promise<RubResult> => {
							await window.lite.assignHunk({
								projectId,
								assignments: source.changes.flatMap(({ change, hunkHeaders }) =>
									hunkHeaders.map(
										(hunkHeader): HunkAssignmentRequest => ({
											pathBytes: change.pathBytes,
											hunkHeader,
											stackId: target.stackId,
										}),
									),
								),
							});
							return {};
						}),
						Match.tag("Commit", async (target): Promise<RubResult> => {
							const response = await window.lite.commitAmend({
								projectId,
								commitId: target.commitId,
								changes: source.changes.map(({ change, hunkHeaders }) =>
									createDiffSpec(change, hunkHeaders),
								),
							});
							return {
								replacedCommits: response.replacedCommits,
								newCommit: response.newCommit ?? null,
								amendedCommitId: target.commitId,
								rejectedChanges: response.rejectedChanges,
							};
						}),
						Match.exhaustive,
					),
				),
				Match.tag("Commit", (sourceParent) =>
					Match.value(target).pipe(
						Match.tag("Changes", async (target): Promise<RubResult> => {
							const response = await window.lite.commitUncommitChanges({
								projectId,
								commitId: sourceParent.commitId,
								assignTo: target.stackId,
								changes: source.changes.map(({ change, hunkHeaders }) =>
									createDiffSpec(change, hunkHeaders),
								),
							});
							return {
								replacedCommits: response.replacedCommits,
							};
						}),
						Match.tag("Commit", async (target): Promise<RubResult> => {
							const response = await window.lite.commitMoveChangesBetween({
								projectId,
								sourceCommitId: sourceParent.commitId,
								destinationCommitId: target.commitId,
								changes: source.changes.map(({ change, hunkHeaders }) =>
									createDiffSpec(change, hunkHeaders),
								),
							});
							return { replacedCommits: response.replacedCommits };
						}),
						Match.exhaustive,
					),
				),
				Match.exhaustive,
			),
		),
		Match.tag("Commit", () =>
			Match.value(target).pipe(
				// TODO: implement when API is ready
				Match.tag("Changes", async (): Promise<RubResult> => {
					throw new Error("Uncommitting has not been implemented yet.");
				}),
				// TODO: implement when API is ready
				Match.tag("Commit", async (): Promise<RubResult> => {
					throw new Error("Squashing has not been implemented yet.");
				}),
				Match.exhaustive,
			),
		),
		Match.exhaustive,
	);
