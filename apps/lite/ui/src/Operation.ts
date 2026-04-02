import { Toast } from "@base-ui/react";
import { useMutation } from "@tanstack/react-query";
import { Match } from "effect";
import {
	type AssignHunkParams,
	type CommitAmendParams,
	type CommitCreateParams,
	type CommitInsertBlankParams,
	type CommitMoveParams,
	type CommitMoveChangesBetweenParams,
	type CommitUncommitChangesParams,
	type MoveBranchParams,
	type TearOffBranchParams,
} from "#electron/ipc.ts";
import { rejectedChangesToastOptions } from "#ui/components/RejectedChanges.tsx";
import {
	assignHunkMutationOptions,
	commitAmendMutationOptions,
	commitCreateMutationOptions,
	commitInsertBlankMutationOptions,
	commitMoveMutationOptions,
	commitMoveChangesBetweenMutationOptions,
	commitSquashMutationOptions,
	commitUncommitChangesMutationOptions,
	commitUncommitMutationOptions,
	moveBranchMutationOptions,
	tearOffBranchMutationOptions,
	CommitSquashParams,
	CommitUncommitParams,
} from "#ui/api/mutations.ts";
import { InsertSide } from "@gitbutler/but-sdk";

export type Operation =
	| ({ _tag: "AssignHunk" } & Omit<AssignHunkParams, "projectId">)
	| ({ _tag: "CommitAmend" } & Omit<CommitAmendParams, "projectId">)
	| ({ _tag: "CommitCreate" } & Omit<CommitCreateParams, "projectId">)
	| ({
			_tag: "CommitCreateFromCommittedChanges";
	  } & Omit<CommitInsertBlankParams, "projectId"> &
			Pick<CommitMoveChangesBetweenParams, "changes" | "sourceCommitId">)
	| ({ _tag: "CommitMove" } & Omit<CommitMoveParams, "projectId">)
	| ({ _tag: "CommitMoveChangesBetween" } & Omit<CommitMoveChangesBetweenParams, "projectId">)
	| ({ _tag: "CommitSquash" } & Omit<CommitSquashParams, "projectId">)
	| ({ _tag: "CommitUncommit" } & Omit<CommitUncommitParams, "projectId">)
	| ({ _tag: "CommitUncommitChanges" } & Omit<CommitUncommitChangesParams, "projectId">)
	| ({ _tag: "MoveBranch" } & Omit<MoveBranchParams, "projectId">)
	| ({ _tag: "TearOffBranch" } & Omit<TearOffBranchParams, "projectId">);

export const isCombineOperation = (operation: Operation): boolean =>
	Match.value(operation).pipe(
		Match.tags({
			AssignHunk: () => true,
			CommitAmend: () => true,
			CommitMoveChangesBetween: () => true,
			CommitSquash: () => true,
			CommitUncommit: () => true,
			CommitUncommitChanges: () => true,
		}),
		Match.orElse(() => false),
	);

export const getInsertionSide = (operation: Operation): InsertSide | null =>
	Match.value(operation).pipe(
		Match.tags({
			CommitMove: (x) => x.side,
			CommitCreate: (x) => x.side,
			CommitCreateFromCommittedChanges: (x) => x.side,
		}),
		Match.orElse(() => null),
	);

export const operationLabel = (operation: Operation): string | null =>
	Match.value(operation).pipe(
		Match.tagsExhaustive({
			AssignHunk: (operation) =>
				operation.assignments[0]?.stackId == null ? "Unassign" : "Assign",
			CommitAmend: () => "Amend",
			CommitCreate: () => "Commit changes here",
			CommitCreateFromCommittedChanges: () => "Create commit here",
			CommitMove: () => "Move commit here",
			CommitMoveChangesBetween: () => "Amend",
			CommitSquash: () => "Squash",
			CommitUncommit: () => "Uncommit",
			CommitUncommitChanges: () => "Uncommit",
			MoveBranch: () => "Stack branch onto here",
			TearOffBranch: () => null,
		}),
	);

export const useRunOperation = (projectId: string) => {
	const toastManager = Toast.useToastManager();
	const assignHunk = useMutation(assignHunkMutationOptions);
	const commitAmend = useMutation(commitAmendMutationOptions);
	const commitCreate = useMutation(commitCreateMutationOptions);
	const commitInsertBlank = useMutation(commitInsertBlankMutationOptions);
	const commitMove = useMutation(commitMoveMutationOptions);
	const commitMoveChangesBetween = useMutation(commitMoveChangesBetweenMutationOptions);
	const commitSquash = useMutation(commitSquashMutationOptions);
	const commitUncommit = useMutation(commitUncommitMutationOptions);
	const commitUncommitChanges = useMutation(commitUncommitChangesMutationOptions);
	const moveBranch = useMutation(moveBranchMutationOptions);
	const tearOffBranch = useMutation(tearOffBranchMutationOptions);

	return (operation: Operation): void => {
		Match.value(operation).pipe(
			Match.tag("AssignHunk", (operation) => {
				assignHunk.mutate({
					projectId,
					assignments: operation.assignments,
				});
			}),
			Match.tag("CommitAmend", (operation) => {
				commitAmend.mutate(
					{
						projectId,
						commitId: operation.commitId,
						changes: operation.changes,
					},
					{
						onSuccess: (response) => {
							if (response.rejectedChanges.length > 0)
								toastManager.add(
									rejectedChangesToastOptions({
										newCommit: response.newCommit ?? null,
										rejectedChanges: response.rejectedChanges,
									}),
								);
						},
					},
				);
			}),
			Match.tag("CommitMoveChangesBetween", (operation) => {
				commitMoveChangesBetween.mutate({
					projectId,
					sourceCommitId: operation.sourceCommitId,
					destinationCommitId: operation.destinationCommitId,
					changes: operation.changes,
				});
			}),
			Match.tag("CommitSquash", (operation) => {
				commitSquash.mutate({
					projectId,
					sourceCommitId: operation.sourceCommitId,
					destinationCommitId: operation.destinationCommitId,
				});
			}),
			Match.tag("CommitUncommit", (operation) => {
				commitUncommit.mutate({
					projectId,
					commitId: operation.commitId,
					assignTo: operation.assignTo,
				});
			}),
			Match.tag("CommitUncommitChanges", (operation) => {
				commitUncommitChanges.mutate({
					projectId,
					commitId: operation.commitId,
					assignTo: operation.assignTo,
					changes: operation.changes,
				});
			}),
			Match.tag("CommitCreate", (operation) => {
				commitCreate.mutate(
					{
						projectId,
						relativeTo: operation.relativeTo,
						side: operation.side,
						changes: operation.changes,
						message: operation.message,
					},
					{
						onSuccess: (response) => {
							if (response.rejectedChanges.length > 0)
								toastManager.add(
									rejectedChangesToastOptions({
										newCommit: response.newCommit,
										rejectedChanges: response.rejectedChanges,
									}),
								);
						},
					},
				);
			}),
			Match.tag("CommitCreateFromCommittedChanges", (operation) => {
				// Ideally this would be an atomic backend operation.
				void (async () => {
					const insertedCommit = await commitInsertBlank.mutateAsync({
						projectId,
						relativeTo: operation.relativeTo,
						side: operation.side,
					});

					await commitMoveChangesBetween.mutateAsync({
						projectId,
						sourceCommitId:
							insertedCommit.replacedCommits[operation.sourceCommitId] ?? operation.sourceCommitId,
						destinationCommitId: insertedCommit.newCommit,
						changes: operation.changes,
					});
				})();
			}),
			Match.tag("CommitMove", (operation) => {
				commitMove.mutate({
					projectId,
					subjectCommitId: operation.subjectCommitId,
					relativeTo: operation.relativeTo,
					side: operation.side,
				});
			}),
			Match.tag("MoveBranch", (operation) => {
				moveBranch.mutate({
					projectId,
					subjectBranch: operation.subjectBranch,
					targetBranch: operation.targetBranch,
				});
			}),
			Match.tag("TearOffBranch", (operation) => {
				tearOffBranch.mutate({
					projectId,
					subjectBranch: operation.subjectBranch,
				});
			}),
			Match.exhaustive,
		);
	};
};
