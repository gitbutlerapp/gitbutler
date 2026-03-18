import {
	AssignmentRejection,
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

/** @public */
export type RubResult = {
	replacedCommits?: Record<string, string>;
	newCommit?: string | null;
	amendedCommitId?: string;
	pathsToRejectedChanges?: UICommitCreateResult["pathsToRejectedChanges"];
	assignmentRejections?: Array<AssignmentRejection>;
};

export type RubParams = {
	projectId: string;
	source: RubSource;
	target: ChangeUnit;
};

export const rub = async ({ projectId, source, target }: RubParams): Promise<RubResult> =>
	Match.value(source).pipe(
		Match.tag("TreeChange", ({ source }) => {
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
								pathsToRejectedChanges: response.pathsToRejectedChanges,
							};
						}),
						Match.tag("changes", async (target): Promise<RubResult> => {
							const response = await window.lite.assignHunk({
								projectId,
								assignments: source.hunkHeaders.map(
									(hunkHeader): HunkAssignmentRequest => ({
										pathBytes: source.change.pathBytes,
										hunkHeader,
										stackId: target.stackId,
									}),
								),
							});
							return { assignmentRejections: response };
						}),
						Match.exhaustive,
					),
				),
				Match.exhaustive,
			);
		}),
		// TODO: implement squashing when API is ready
		Match.tag("Commit", async (): Promise<RubResult> => {
			throw new Error("Squashing has not been implemented yet.");
		}),
		Match.exhaustive,
	);

type RubOperationLabel = "Amend" | "Uncommit" | "Assign" | "Unassign" | "Squash";

export const rubOperationLabel = (
	rubSource: RubSource,
	target: ChangeUnit,
): RubOperationLabel | null =>
	Match.value(rubSource).pipe(
		Match.withReturnType<RubOperationLabel | null>(),
		Match.tag("TreeChange", ({ source }) =>
			Match.value(source.parent).pipe(
				Match.withReturnType<RubOperationLabel | null>(),
				Match.tag("commit", (source) =>
					Match.value(target).pipe(
						Match.withReturnType<RubOperationLabel | null>(),
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
						Match.withReturnType<RubOperationLabel | null>(),
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
		Match.tag("Commit", ({ source }) =>
			Match.value(target).pipe(
				Match.withReturnType<RubOperationLabel | null>(),
				Match.tag("commit", (target) => {
					if (source.commitId === target.commitId) return null;
					return "Squash";
				}),
				Match.tag("changes", () => null),
				Match.exhaustive,
			),
		),
		Match.exhaustive,
	);
