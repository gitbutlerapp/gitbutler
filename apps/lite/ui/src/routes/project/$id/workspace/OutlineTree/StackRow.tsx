import { useUnapplyStack, useWorkspaceIntegrateUpstream } from "#ui/api/mutations.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { classes } from "#ui/components/classes.ts";
import { outlineHotkeys, toElectronAccelerator } from "#ui/hotkeys.ts";
import {
	nativeMenuItem,
	nativeMenuSeparator,
	showNativeContextMenu,
	showNativeMenuFromTrigger,
	type NativeMenuItem,
} from "#ui/native-menu.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { stackBottomRelativeTo } from "#ui/api/stack.ts";
import { decodeBytes } from "#ui/api/bytes.ts";
import { Toolbar } from "@base-ui/react";
import type { BottomUpdate, Stack } from "@gitbutler/but-sdk";
import type { ComponentProps, FC } from "react";
import { getRowButtonClassName } from "../Row-utils.ts";
import { StackCardHeader, StackFoldAllButton } from "../StackCard.tsx";
import styles from "./StackRow.module.css";

export const StackRow: FC<
	{
		projectId: string;
		stack: Stack;
	} & Omit<ComponentProps<"div">, "onSelect">
> = ({ projectId, stack, ...restProps }) => {
	const relativeTo = stackBottomRelativeTo(stack);
	const rebaseUpdate: BottomUpdate | null = relativeTo
		? { kind: "rebase", selector: relativeTo }
		: null;
	const isDefaultMode = useAppSelector(
		(state) => projectSlice.selectors.selectOutlineModeState(state, projectId)._tag === "Default",
	);

	const dispatch = useAppDispatch();
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

	const { isPending: isUnapplyStackPending, mutate: unapplyStack } = useUnapplyStack();
	const unapply = () => {
		// In the future we should have an unapply API that doesn't require an ID.
		if (stack.id === null) throw new Error("Require stack ID in order to unapply");

		unapplyStack({ projectId, stackId: stack.id });
	};

	const { mutate: workspaceIntegrateUpstream } = useWorkspaceIntegrateUpstream();
	const updateStack = () => {
		if (rebaseUpdate) {
			workspaceIntegrateUpstream({
				projectId,
				updates: [rebaseUpdate],
				dryRun: false,
			});
		}
	};

	const menuItems: Array<NativeMenuItem> = [
		nativeMenuItem({ label: "Move Up", enabled: false }),
		nativeMenuItem({ label: "Move Down", enabled: false }),
		nativeMenuSeparator,
		nativeMenuItem({
			label: "Update Stack (Rebases)",
			enabled: !!rebaseUpdate,
			accelerator: toElectronAccelerator(outlineHotkeys.updateStack.hotkey),
			onSelect: updateStack,
		}),
		nativeMenuItem({
			label: "Unapply Stack",
			enabled: !isUnapplyStackPending,
			onSelect: unapply,
		}),
	];

	return (
		<StackCardHeader
			{...restProps}
			toolbarLabel="Stack actions"
			onContextMenu={(event) => {
				void showNativeContextMenu(event, menuItems);
			}}
		>
			<StackFoldAllButton
				hasMultipleBranches={branchCount > 1}
				folded={anyFolded}
				disabled={foldableRefs.length === 0}
				onToggle={() =>
					dispatch(
						projectSlice.actions.setSegmentsFolded({
							projectId,
							branchRefs: foldableRefs,
							folded: !anyFolded,
						}),
					)
				}
			/>

			<span
				aria-hidden
				data-disabled={!isDefaultMode || undefined}
				className={classes(getRowButtonClassName({ iconOnly: true }), styles.moveIndicator)}
			>
				<Icon name="drag-square" />
			</span>

			<Toolbar.Button
				aria-label="Stack menu"
				disabled={!isDefaultMode}
				onClick={(event) => {
					void showNativeMenuFromTrigger(event.currentTarget, menuItems);
				}}
				className={getRowButtonClassName({ iconOnly: true })}
			>
				<Icon name="kebab" />
			</Toolbar.Button>
		</StackCardHeader>
	);
};
