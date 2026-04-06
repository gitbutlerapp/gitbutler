import {
	changesInWorktreeQueryOptions,
	commitDetailsWithLineStatsQueryOptions,
} from "#ui/api/queries.ts";
import { type Operation } from "#ui/Operation.ts";
import { createDiffSpec } from "#ui/domain/DiffSpec.ts";
import { type FileParent } from "#ui/domain/FileParent.ts";
import { useQueryClient } from "@tanstack/react-query";
import {
	CommitDetails,
	WorktreeChanges,
	type HunkAssignment,
	type HunkHeader,
	type TreeChange,
} from "@gitbutler/but-sdk";
import { Match } from "effect";
import { decodeRefName, getAssignmentsByPath } from "../-shared";
import { type OperationSourceRef } from "./-OperationSourceRef.ts";

export type TreeChangeWithHunkHeaders = {
	change: TreeChange;
	hunkHeaders: Array<HunkHeader>;
};

export type OperationSource =
	| { _tag: "Commit"; commitId: string }
	| { _tag: "Branch"; ref: Array<number> }
	| {
			_tag: "TreeChanges";
			parent: FileParent;
			changes: Array<TreeChangeWithHunkHeaders>;
	  };

const hunkHeadersForAssignments = (
	assignments: Array<HunkAssignment> | undefined,
): Array<HunkHeader> =>
	assignments
		? assignments.flatMap((assignment) =>
				assignment.hunkHeader != null ? [assignment.hunkHeader] : [],
			)
		: [];

const treeChangesOperationSource = ({
	parent,
	changes,
}: {
	parent: FileParent;
	changes: Array<TreeChangeWithHunkHeaders>;
}): OperationSource => ({
	_tag: "TreeChanges",
	parent,
	changes,
});

export const resolveOperationSource = ({
	operationSourceRef,
	worktreeChanges,
	getCommitDetails,
}: {
	operationSourceRef: OperationSourceRef;
	worktreeChanges: WorktreeChanges | undefined;
	getCommitDetails: (commitId: string) => CommitDetails | undefined;
}): OperationSource | null =>
	Match.value(operationSourceRef).pipe(
		Match.tagsExhaustive({
			Branch: (operationSourceRef): OperationSource => ({
				_tag: "Branch",
				ref: operationSourceRef.ref,
			}),
			Commit: (operationSourceRef): OperationSource => ({
				_tag: "Commit",
				commitId: operationSourceRef.commitId,
			}),
			ChangesSection: ({ stackId }): OperationSource | null => {
				if (!worktreeChanges) return null;

				const assignmentsByPath = getAssignmentsByPath(worktreeChanges.assignments, stackId);
				const changes = worktreeChanges.changes.flatMap((change) => {
					const assignments = assignmentsByPath.get(change.path);
					if (!assignments) return [];

					return [
						{
							change,
							hunkHeaders: hunkHeadersForAssignments(assignments),
						},
					];
				});

				return treeChangesOperationSource({
					parent: { _tag: "ChangesSection", stackId },
					changes,
				});
			},
			File: ({ parent, path }): OperationSource | null => {
				const change = Match.value(parent).pipe(
					Match.tag("ChangesSection", () => {
						if (!worktreeChanges) return null;

						return worktreeChanges.changes.find((candidate) => candidate.path === path) ?? null;
					}),
					Match.tag("Commit", ({ commitId }) => {
						const commitDetails = getCommitDetails(commitId);
						if (!commitDetails) return null;

						return commitDetails.changes.find((candidate) => candidate.path === path) ?? null;
					}),
					Match.exhaustive,
				);

				if (!change) return null;

				const hunkHeaders = Match.value(parent).pipe(
					Match.tag("ChangesSection", ({ stackId }) => {
						if (!worktreeChanges) return [];

						return hunkHeadersForAssignments(
							getAssignmentsByPath(worktreeChanges.assignments, stackId).get(path),
						);
					}),
					Match.tag("Commit", () => []),
					Match.exhaustive,
				);

				return treeChangesOperationSource({
					parent,
					changes: [{ change, hunkHeaders }],
				});
			},
			Hunk: ({ parent, path, hunkHeader }): OperationSource | null => {
				const change = Match.value(parent).pipe(
					Match.tag("ChangesSection", () => {
						if (!worktreeChanges) return null;

						return worktreeChanges.changes.find((candidate) => candidate.path === path) ?? null;
					}),
					Match.tag("Commit", ({ commitId }) => {
						const commitDetails = getCommitDetails(commitId);
						if (!commitDetails) return null;

						return commitDetails.changes.find((candidate) => candidate.path === path) ?? null;
					}),
					Match.exhaustive,
				);

				if (!change) return null;

				return treeChangesOperationSource({
					parent,
					changes: [{ change, hunkHeaders: [hunkHeader] }],
				});
			},
		}),
	);

export const useResolveOperationSource = (projectId: string) => {
	const queryClient = useQueryClient();

	return (operationSourceRef: OperationSourceRef): OperationSource | null =>
		resolveOperationSource({
			operationSourceRef,
			worktreeChanges: queryClient.getQueryData(changesInWorktreeQueryOptions(projectId).queryKey),
			getCommitDetails: (commitId) =>
				queryClient.getQueryData(
					commitDetailsWithLineStatsQueryOptions({ projectId, commitId }).queryKey,
				),
		});
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
	operationSource,
	target,
}: {
	operationSource: OperationSource;
	target: FileParent;
}): Operation | null =>
	Match.value(operationSource).pipe(
		Match.tagsExhaustive({
			Branch: (): Operation | null => null,
			Commit: ({ commitId: sourceCommitId }) =>
				Match.value(target).pipe(
					Match.tagsExhaustive({
						ChangesSection: ({ stackId }): Operation => ({
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
						ChangesSection: ({ stackId: sourceStackId }) =>
							Match.value(target).pipe(
								Match.tagsExhaustive({
									ChangesSection: ({ stackId: targetStackId }): Operation | null => {
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
									ChangesSection: ({ stackId }): Operation => ({
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
	operationSource,
	branchRef,
	firstCommitId,
}: {
	operationSource: OperationSource;
	branchRef: Array<number> | null;
	firstCommitId: string | undefined;
}): Operation | null =>
	Match.value(operationSource).pipe(
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
			if (branchRef === null || source.parent._tag !== "ChangesSection") return null;
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
	operationSource,
	commitId,
	action,
}: {
	operationSource: OperationSource;
	commitId: string;
	action: CommitTargetAction;
}): Operation | null =>
	Match.value(action).pipe(
		Match.when("combine", (): Operation | null =>
			getCombineOperation({
				operationSource,
				target: { _tag: "Commit", commitId },
			}),
		),
		Match.whenOr("insertAbove", "insertBelow", (action): Operation | null => {
			const side = action === "insertAbove" ? "above" : "below";

			if (operationSource._tag === "Commit")
				return {
					_tag: "CommitMove",
					subjectCommitId: operationSource.commitId,
					relativeTo: { type: "commit", subject: commitId },
					side,
				};

			if (
				operationSource._tag === "TreeChanges" &&
				operationSource.parent._tag === "ChangesSection"
			)
				return {
					_tag: "CommitCreate",
					relativeTo: { type: "commit", subject: commitId },
					side,
					changes: operationSource.changes.map(({ change, hunkHeaders }) =>
						createDiffSpec(change, hunkHeaders),
					),
					message: "",
				};

			if (operationSource._tag === "TreeChanges" && operationSource.parent._tag === "Commit")
				return {
					_tag: "CommitCreateFromCommittedChanges",
					sourceCommitId: operationSource.parent.commitId,
					relativeTo: { type: "commit", subject: commitId },
					side,
					changes: operationSource.changes.map(({ change, hunkHeaders }) =>
						createDiffSpec(change, hunkHeaders),
					),
				};

			return null;
		}),
		Match.exhaustive,
	);
