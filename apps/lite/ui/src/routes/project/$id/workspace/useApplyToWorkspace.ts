import { useApply } from "#ui/api/mutations.ts";
import { setCursor, setPage } from "#ui/use-cursor.ts";
import { encodeBytes } from "#ui/api/bytes.ts";
import { branchOperand } from "#ui/operands.ts";

/**
 * Apply a branch and follow it into the workspace: on success the outline
 * switches to the workspace tab with the applied branch selected, so the
 * details pane stays on the branch the user was looking at.
 */
export const useApplyToWorkspace = (projectId: string) => {
	const { isPending, mutate } = useApply();

	const apply = (branchRef: string) => {
		mutate(
			{ projectId, existingBranch: branchRef },
			{
				// A conflicting apply succeeds with nothing applied; useApply
				// already toasts it, so there is nothing to follow.
				onSuccess: (response) => {
					const appliedRef = response.appliedBranches[0];
					if (!appliedRef) return;

					setPage("workspace");
					setCursor("stacks", branchOperand({ branchRef: encodeBytes(appliedRef.full) }));
				},
			},
		);
	};

	return { isPending, apply };
};
