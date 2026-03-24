import { Toast } from "@base-ui/react";
import { useMutation } from "@tanstack/react-query";
import { Match } from "effect";
import { CommitMoveParams, MoveBranchParams, TearOffBranchParams } from "#electron/ipc.ts";
import { RejectedChange, rejectedChangesToastOptions } from "#ui/components/RejectedChanges.tsx";
import {
	commitMoveMutationOptions,
	moveBranchMutationOptions,
	rubMutationOptions,
	tearOffBranchMutationOptions,
} from "#ui/api/mutations.ts";
import { RubParams } from "#ui/api/rub.ts";

export type RubOperation = Omit<RubParams, "projectId">;

export type Operation =
	| ({
			_tag: "Rub";
	  } & RubOperation)
	| ({
			_tag: "CommitMove";
	  } & Omit<CommitMoveParams, "projectId">)
	| ({
			_tag: "MoveBranch";
	  } & Omit<MoveBranchParams, "projectId">)
	| ({
			_tag: "TearOffBranch";
	  } & Omit<TearOffBranchParams, "projectId">);

export const useRunOperation = (projectId: string) => {
	const toastManager = Toast.useToastManager();
	const rubMutation = useMutation(rubMutationOptions);
	const commitMove = useMutation(commitMoveMutationOptions);
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
							const pathsToRejectedChanges = response.pathsToRejectedChanges ?? [];
							if (pathsToRejectedChanges.length > 0)
								toastManager.add(
									rejectedChangesToastOptions({
										newCommit: response.newCommit,
										pathsToRejectedChanges:
											response.pathsToRejectedChanges as Array<RejectedChange>,
									}),
								);
							return;
						},
					},
				);
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
