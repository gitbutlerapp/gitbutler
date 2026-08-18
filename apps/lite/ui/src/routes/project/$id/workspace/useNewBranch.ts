import { useBranchCheckoutNew, useBranchCreate } from "#ui/api/mutations.ts";
import { toElectronAccelerator, workspaceHotkeys } from "#ui/hotkeys.ts";
import { nativeMenuItem, type NativeMenuItem } from "#ui/native-menu.ts";
import { branchOperand } from "#ui/operands.ts";
import { focusScope } from "#ui/focus-scopes.ts";
import { setCursor, setPage } from "#ui/use-cursor.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppSelector } from "#ui/store.ts";

export type NewBranchActions = {
	menuItems: Array<NativeMenuItem>;
	/** Whether either branch is being created, for the trigger's spinner. */
	isPending: boolean;
	/** Whether a branch can be started at all, for the trigger's disabled state. */
	enabled: boolean;
	/** The plain create, for the hotkey that skips the menu. */
	createInWorkspace: () => void;
	createAndSwitch: () => void;
};

/**
 * The two ways to start a branch, and the difference between them is the whole
 * workspace: one adds a lane beside what is already applied, the other leaves
 * for it the way a plain checkout does. Neither is the obvious default, so
 * every `+` offers both rather than picking.
 *
 * Call this once, in the outline, and hand the result to every trigger: two
 * instances would each hold their own mutation, so neither one's in-flight
 * guard would see the other's create and a hotkey could start a second branch
 * while a menu's is still landing.
 */
export const useNewBranch = (projectId: string): NewBranchActions => {
	// Read here rather than taken from callers: an outline busy with a transfer
	// or an absorb is no place to start a branch from, and that is true of every
	// trigger rather than something each one should have to remember.
	const isDefaultMode = useAppSelector(
		(state) => projectSlice.selectors.selectOutlineModeState(state, projectId)._tag === "Default",
	);
	const { isPending: isCreatePending, mutate: branchCreate } = useBranchCreate();
	const { isPending: isCheckoutPending, mutate: branchCheckoutNew } = useBranchCheckoutNew();

	const createInWorkspace = () => {
		branchCreate(
			{ projectId, newRef: null, placement: { type: "independent" } },
			{
				onSuccess: (response) => {
					// The new branch is a workspace lane now, so the workspace tab is
					// where it can be seen — and selecting it there is what opens it for
					// renaming, which is the first thing a canned name wants.
					setPage("workspace");
					setCursor("stacks", branchOperand({ branchRef: response.newRef.fullNameBytes }));
					focusScope("outline");
				},
			},
		);
	};

	const createAndSwitch = () => {
		// Nothing to select afterwards: the checkout leaves the workspace behind,
		// so there is no lane for the new branch to be.
		branchCheckoutNew({ projectId, name: null }, { onSuccess: () => setPage("workspace") });
	};

	return {
		menuItems: [
			nativeMenuItem({
				label: "New Branch in Workspace",
				enabled: isDefaultMode && !isCreatePending,
				accelerator: toElectronAccelerator(workspaceHotkeys.createIndependentBranch.hotkey),
				onSelect: createInWorkspace,
			}),
			nativeMenuItem({
				label: "New Branch and Switch to It",
				enabled: isDefaultMode && !isCheckoutPending,
				accelerator: toElectronAccelerator(workspaceHotkeys.createBranchAndSwitch.hotkey),
				onSelect: createAndSwitch,
			}),
		],
		isPending: isCreatePending || isCheckoutPending,
		enabled: isDefaultMode && !isCreatePending && !isCheckoutPending,
		createInWorkspace,
		createAndSwitch,
	};
};
