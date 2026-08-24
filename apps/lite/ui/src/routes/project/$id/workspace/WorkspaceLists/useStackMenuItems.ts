import { useUnapplyStack, useWorkspaceIntegrateUpstream } from "#ui/api/mutations.ts";
import { decodeBytes } from "#ui/api/bytes.ts";
import { operatingModeQueryOptions } from "#ui/api/queries.ts";
import { stackBottomRelativeTo } from "#ui/api/stack.ts";
import { sidebarHotkeys, toElectronAccelerator } from "#ui/hotkeys.ts";
import { nativeMenuItem, type NativeMenuItem } from "#ui/native-menu.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import type { BottomUpdate, Stack } from "@gitbutler/but-sdk";
import { useQuery } from "@tanstack/react-query";

/**
 * The actions that operate on a whole stack rather than one of its branches.
 *
 * The stack has no row of its own, so these hang off the menu of every branch
 * in it — as their own group, since a branch's own actions ("Delete Branch
 * Reference") read very differently from the stack-wide ones next to them.
 */
export const useStackMenuItems = (projectId: string, stack: Stack): Array<NativeMenuItem> => {
	const dispatch = useAppDispatch();
	const { data: isOpenWorkspace } = useQuery({
		...operatingModeQueryOptions(projectId),
		select: (headAndMode) => headAndMode.operatingMode.type === "OpenWorkspace",
	});
	const noOperationPending = useAppSelector(
		(state) => projectSlice.selectors.selectPendingOperation(state, projectId)._tag === "None",
	);

	const branchCount = stack.segments.filter((segment) => segment.refName !== null).length;
	// Only a segment with a branch reference and commits to hide can be folded;
	// fold state is keyed by that reference.
	const foldableRefs = stack.segments.flatMap((segment) =>
		segment.refName !== null && segment.commits.length > 0
			? [decodeBytes(segment.refName.fullNameBytes)]
			: [],
	);
	// A plain boolean, so this re-renders only when the stack crosses between
	// fully unfolded and not.
	const anyFolded = useAppSelector((state) =>
		foldableRefs.some((branchRef) =>
			projectSlice.selectors.selectSegmentFolded(state, projectId, branchRef),
		),
	);

	const relativeTo = stackBottomRelativeTo(stack);
	const rebaseUpdate: BottomUpdate | null = relativeTo
		? { kind: "rebase", selector: relativeTo }
		: null;

	const { isPending: isUnapplyStackPending, mutate: unapplyStack } = useUnapplyStack();
	const { mutate: workspaceIntegrateUpstream } = useWorkspaceIntegrateUpstream();

	return [
		// The fold items stay reachable outside the default mode: folding is a
		// view operation, and the items that mutate the stack gate themselves.
		nativeMenuItem({
			label: anyFolded ? "Unfold All Branches In Stack" : "Fold All Branches In Stack",
			enabled: branchCount > 1 && foldableRefs.length > 0,
			onSelect: () => {
				dispatch(
					projectSlice.actions.setSegmentsFolded({
						projectId,
						branchRefs: foldableRefs,
						folded: !anyFolded,
					}),
				);
			},
		}),
		nativeMenuItem({
			label: "Update Stack (Rebases)",
			enabled: noOperationPending && !!rebaseUpdate,
			accelerator: toElectronAccelerator(sidebarHotkeys.updateStack.hotkey),
			onSelect: () => {
				if (rebaseUpdate) {
					workspaceIntegrateUpstream({
						projectId,
						updates: [rebaseUpdate],
						dryRun: false,
					});
				}
			},
		}),
		nativeMenuItem({
			label: "Unapply Stack",
			enabled: isOpenWorkspace === true && noOperationPending && !isUnapplyStackPending,
			onSelect: () => {
				// In the future we should have an unapply API that doesn't require an ID.
				if (stack.id === null) throw new Error("Require stack ID in order to unapply");

				unapplyStack({ projectId, stackId: stack.id });
			},
		}),
	];
};
