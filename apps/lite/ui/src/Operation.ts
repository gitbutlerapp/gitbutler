import { Toast } from "@base-ui/react";
import { useMutation } from "@tanstack/react-query";
import { Match } from "effect";
import {
	CommitCreateParams,
	CommitInsertBlankParams,
	CommitMoveParams,
	CommitMoveChangesBetweenParams,
	MoveBranchParams,
	TearOffBranchParams,
} from "#electron/ipc.ts";
import { rejectedChangesToastOptions } from "#ui/components/RejectedChanges.tsx";
import {
	commitCreateMutationOptions,
	commitInsertBlankMutationOptions,
	commitMoveMutationOptions,
	commitMoveChangesBetweenMutationOptions,
	moveBranchMutationOptions,
	rubMutationOptions,
	tearOffBranchMutationOptions,
} from "#ui/api/mutations.ts";
import { RubParams } from "#ui/api/rub.ts";

export type RubOperation = Omit<RubParams, "projectId">;

export type Operation =
	| ({ _tag: "Rub" } & RubOperation)
	| ({ _tag: "CommitCreate" } & Omit<CommitCreateParams, "projectId">)
	| ({
			_tag: "CommitCreateFromCommittedChanges";
	  } & Omit<CommitInsertBlankParams, "projectId"> &
			Pick<CommitMoveChangesBetweenParams, "changes" | "sourceCommitId">)
	| ({ _tag: "CommitMove" } & Omit<CommitMoveParams, "projectId">)
	| ({ _tag: "MoveBranch" } & Omit<MoveBranchParams, "projectId">)
	| ({ _tag: "TearOffBranch" } & Omit<TearOffBranchParams, "projectId">);

export const useRunOperation = (projectId: string) => {
	const toastManager = Toast.useToastManager();
	const rubMutation = useMutation(rubMutationOptions);
	const commitCreate = useMutation(commitCreateMutationOptions);
	const commitInsertBlank = useMutation(commitInsertBlankMutationOptions);
	const commitMove = useMutation(commitMoveMutationOptions);
	const commitMoveChangesBetween = useMutation(commitMoveChangesBetweenMutationOptions);
	const moveBranch = useMutation(moveBranchMutationOptions);
	const tearOffBranch = useMutation(tearOffBranchMutationOptions);

	return (operation: Operation): void => {
		Match.value(operation).pipe(
			Match.tag("Rub", (operation) => {
				rubMutation.mutate(
					{
						projectId,
						source: operation.source,
						target: operation.target,
					},
					{
						onSuccess: (response) => {
							const rejectedChanges = response.rejectedChanges ?? [];
							if (rejectedChanges.length > 0)
								toastManager.add(
									rejectedChangesToastOptions({
										newCommit: response.newCommit,
										rejectedChanges,
									}),
								);
							return;
						},
					},
				);
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
