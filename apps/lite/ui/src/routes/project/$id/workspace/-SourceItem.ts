import { type Operation } from "#ui/Operation.ts";
import { createDiffSpec } from "#ui/domain/DiffSpec.ts";
import { type ChangeUnit } from "#ui/domain/ChangeUnit.ts";
import { type HunkHeader, type TreeChange } from "@gitbutler/but-sdk";
import { Match } from "effect";
import { decodeRefName } from "../-shared";

export type TreeChangeWithHunkHeaders = {
	change: TreeChange;
	hunkHeaders: Array<HunkHeader>;
};

export type SourceItem =
	| { _tag: "Commit"; commitId: string }
	| { _tag: "Branch"; ref: Array<number> }
	| {
			_tag: "TreeChanges";
			parent: ChangeUnit;
			changes: Array<TreeChangeWithHunkHeaders>;
	  };

/**
 * | SOURCE ↓ / TARGET →    | Changes  | Commit |
 * | ---------------------- | -------- | ------ |
 * | File/hunk from changes | Assign   | Amend  |
 * | File/hunk from commit  | Uncommit | Amend  |
 * | Commit                 | Uncommit | Squash |
 *
 * Note this is currently different from the CLI's definition of "rubbing",
 * which also includes move operations.
 * https://linear.app/gitbutler/issue/GB-1160/what-should-rubbing-a-branch-into-another-branch-do#comment-db2abdb7
 */
export const getCombineOperation = ({
	sourceItem,
	target,
}: {
	sourceItem: SourceItem;
	target: ChangeUnit;
}): Operation | null =>
	Match.value(sourceItem).pipe(
		Match.tagsExhaustive({
			Branch: (): Operation | null => null,
			Commit: ({ commitId: sourceCommitId }) =>
				Match.value(target).pipe(
					Match.tagsExhaustive({
						Changes: ({ stackId }): Operation => ({
							_tag: "CommitUncommit",
							commitId: sourceCommitId,
							assignTo: stackId,
						}),
						Commit: ({ commitId: destinationCommitId }): Operation | null => {
							if (sourceCommitId === destinationCommitId) return null;
							return {
								_tag: "CommitSquash",
								sourceCommitId,
								destinationCommitId,
							};
						},
					}),
				),
			TreeChanges: ({ parent, changes: sourceChanges }) => {
				const changes = sourceChanges.map(({ change, hunkHeaders }) =>
					createDiffSpec(change, hunkHeaders),
				);

				return Match.value(parent).pipe(
					Match.tagsExhaustive({
						Changes: ({ stackId: sourceStackId }) =>
							Match.value(target).pipe(
								Match.tagsExhaustive({
									Changes: ({ stackId: targetStackId }): Operation | null => {
										if (sourceStackId === targetStackId) return null;
										return {
											_tag: "AssignHunk",
											assignments: sourceChanges.flatMap(({ change, hunkHeaders }) =>
												hunkHeaders.map((hunkHeader) => ({
													pathBytes: change.pathBytes,
													hunkHeader,
													stackId: targetStackId,
												})),
											),
										};
									},
									Commit: ({ commitId }): Operation => ({
										_tag: "CommitAmend",
										commitId,
										changes,
									}),
								}),
							),
						Commit: ({ commitId: sourceCommitId }) =>
							Match.value(target).pipe(
								Match.tagsExhaustive({
									Changes: ({ stackId }): Operation => ({
										_tag: "CommitUncommitChanges",
										commitId: sourceCommitId,
										assignTo: stackId,
										changes,
									}),
									Commit: ({ commitId: destinationCommitId }): Operation | null => {
										if (sourceCommitId === destinationCommitId) return null;
										return {
											_tag: "CommitMoveChangesBetween",
											sourceCommitId,
											destinationCommitId,
											changes,
										};
									},
								}),
							),
					}),
				);
			},
		}),
	);

export const getBranchTargetOperation = ({
	sourceItem,
	branchRef,
	firstCommitId,
}: {
	sourceItem: SourceItem;
	branchRef: Array<number> | null;
	firstCommitId: string | undefined;
}): Operation | null =>
	Match.value(sourceItem).pipe(
		Match.tag("Branch", (source): Operation | null => {
			if (branchRef === null || decodeRefName(branchRef) === decodeRefName(source.ref)) return null;
			return {
				_tag: "MoveBranch",
				subjectBranch: decodeRefName(source.ref),
				targetBranch: decodeRefName(branchRef),
			};
		}),
		Match.tag("Commit", ({ commitId }): Operation | null => {
			if (branchRef === null || commitId === firstCommitId) return null;
			return {
				_tag: "CommitMove",
				subjectCommitId: commitId,
				relativeTo: {
					type: "referenceBytes",
					subject: branchRef,
				},
				side: "below",
			};
		}),
		Match.tag("TreeChanges", (source): Operation | null => {
			if (branchRef === null || source.parent._tag !== "Changes") return null;
			return {
				_tag: "CommitCreate",
				relativeTo: {
					type: "referenceBytes",
					subject: branchRef,
				},
				side: "below",
				changes: source.changes.map(({ change, hunkHeaders }) =>
					createDiffSpec(change, hunkHeaders),
				),
				message: "",
			};
		}),
		Match.orElse(() => null),
	);

export type CommitTargetAction = "combine" | "insertAbove" | "insertBelow";

export const getCommitTargetOperation = ({
	sourceItem,
	commitId,
	action,
}: {
	sourceItem: SourceItem;
	commitId: string;
	action: CommitTargetAction;
}): Operation | null =>
	Match.value(action).pipe(
		Match.when("combine", (): Operation | null =>
			getCombineOperation({
				sourceItem,
				target: { _tag: "Commit", commitId },
			}),
		),
		Match.whenOr("insertAbove", "insertBelow", (action): Operation | null => {
			const side = action === "insertAbove" ? "above" : "below";

			if (sourceItem._tag === "Commit")
				return {
					_tag: "CommitMove",
					subjectCommitId: sourceItem.commitId,
					relativeTo: { type: "commit", subject: commitId },
					side,
				};

			if (sourceItem._tag === "TreeChanges" && sourceItem.parent._tag === "Changes")
				return {
					_tag: "CommitCreate",
					relativeTo: { type: "commit", subject: commitId },
					side,
					changes: sourceItem.changes.map(({ change, hunkHeaders }) =>
						createDiffSpec(change, hunkHeaders),
					),
					message: "",
				};

			if (sourceItem._tag === "TreeChanges" && sourceItem.parent._tag === "Commit")
				return {
					_tag: "CommitCreateFromCommittedChanges",
					sourceCommitId: sourceItem.parent.commitId,
					relativeTo: { type: "commit", subject: commitId },
					side,
					changes: sourceItem.changes.map(({ change, hunkHeaders }) =>
						createDiffSpec(change, hunkHeaders),
					),
				};

			return null;
		}),
		Match.exhaustive,
	);
