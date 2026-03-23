import {
	HunkAssignmentRequest,
	HunkHeader,
	TreeChange,
	UICommitCreateResult,
} from "@gitbutler/but-sdk";
import { Match } from "effect";
import { type ChangeUnit } from "#ui/ChangeUnit.ts";
import { createDiffSpec } from "#ui/DiffSpec.ts";

export type TreeChangeRubSource = {
	parent: ChangeUnit;
	change: TreeChange;
	hunkHeaders: Array<HunkHeader>;
};

export type CommitRubSource = {
	commitId: string;
};

export type RubSource =
	| { _tag: "TreeChange"; source: TreeChangeRubSource }
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
	pathsToRejectedChanges?: UICommitCreateResult["pathsToRejectedChanges"];
};

export const rub = async ({ projectId, source, target }: RubParams): Promise<RubResult> =>
	Match.value(source).pipe(
		Match.tag("TreeChange", ({ source }) => {
			const changes = [createDiffSpec(source.change, source.hunkHeaders)];

			return Match.value(source.parent).pipe(
				Match.tag("Commit", (source) =>
					Match.value(target).pipe(
						Match.tag("Commit", async (target): Promise<RubResult> => {
							const response = await window.lite.commitMoveChangesBetween({
								projectId,
								sourceCommitId: source.commitId,
								destinationCommitId: target.commitId,
								changes,
							});
							return { replacedCommits: response.replacedCommits };
						}),
						Match.tag("Changes", async (target): Promise<RubResult> => {
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
				Match.tag("Changes", () =>
					Match.value(target).pipe(
						Match.tag("Commit", async (target): Promise<RubResult> => {
							const response = await window.lite.commitAmend({
								projectId,
								commitId: target.commitId,
								changes,
							});
							return {
								replacedCommits: response.replacedCommits,
								newCommit: response.newCommit ?? null,
								amendedCommitId: target.commitId,
								pathsToRejectedChanges: response.pathsToRejectedChanges,
							};
						}),
						Match.tag("Changes", async (target): Promise<RubResult> => {
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
		Match.tag("Commit", () =>
			Match.value(target).pipe(
				// TODO: implement when API is ready
				Match.tag("Commit", async (): Promise<RubResult> => {
					throw new Error("Squashing has not been implemented yet.");
				}),
				// TODO: implement when API is ready
				Match.tag("Changes", async (): Promise<RubResult> => {
					throw new Error("Uncommitting has not been implemented yet.");
				}),
				Match.exhaustive,
			),
		),
		Match.exhaustive,
	);
