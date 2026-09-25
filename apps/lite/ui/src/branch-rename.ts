import { decodeBytes } from "#ui/api/bytes.ts";
import { branchDetailsParams } from "#ui/branch.ts";
import { moveBranchChecklist, moveDraftPR } from "#ui/pr.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { moveBranchReviewedFiles } from "#ui/reviewed-files.ts";
import type { AppDispatch } from "#ui/store.ts";
import { remapSearchBranch } from "#ui/use-cursor.ts";
import type { QueryClient } from "@tanstack/react-query";

/** Carry all client state keyed by a branch's name or ref over to its new one. */
export const applyBranchRename = ({
	queryClient,
	dispatch,
	projectId,
	oldRef,
	newRef,
}: {
	queryClient: QueryClient;
	dispatch: AppDispatch;
	projectId: string;
	oldRef: Array<number>;
	newRef: Array<number>;
}): void => {
	const renamed = {
		queryClient,
		projectId,
		// https://linear.app/gitbutler/issue/GB-1226/unify-branch-identifiers
		oldBranch: branchDetailsParams(decodeBytes(oldRef)).branchName,
		newBranch: branchDetailsParams(decodeBytes(newRef)).branchName,
	};
	moveDraftPR(renamed);
	void moveBranchChecklist(renamed);
	moveBranchReviewedFiles({ queryClient, projectId, oldBranchRef: oldRef, newBranchRef: newRef });
	dispatch(
		projectSlice.actions.updateRewrittenBranchReferences({
			projectId,
			oldBranch: { branchRef: oldRef },
			newBranch: { branchRef: newRef },
		}),
	);
	remapSearchBranch(decodeBytes(oldRef), decodeBytes(newRef));
};
