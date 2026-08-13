import { useApply } from "#ui/api/mutations.ts";
import { encodeBytes } from "#ui/api/bytes.ts";
import { branchOperand } from "#ui/operands.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppDispatch } from "#ui/store.ts";

/**
 * Apply a branch and follow it into the workspace: on success the outline
 * switches to the workspace tab with the applied branch selected, so the
 * details pane stays on the branch the user was looking at.
 */
export const useApplyToWorkspace = (projectId: string) => {
	const dispatch = useAppDispatch();
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

					dispatch(projectSlice.actions.setOutlineTab({ projectId, tab: "workspace" }));
					dispatch(
						projectSlice.actions.selectOutline({
							projectId,
							selection: branchOperand({ branchRef: encodeBytes(appliedRef.full) }),
						}),
					);
				},
			},
		);
	};

	return { isPending, apply };
};
