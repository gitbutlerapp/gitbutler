import { useUnapplyStack, useWorkspaceIntegrateUpstream } from "#ui/api/mutations.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
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
import { useAppSelector } from "#ui/store.ts";
import { stackBottomRelativeTo } from "#ui/api/stack.ts";
import { Tooltip, Toolbar } from "@base-ui/react";
import type { BottomUpdate, Stack } from "@gitbutler/but-sdk";
import type { ComponentProps, FC } from "react";
import { getRowButtonClassName } from "../Row-utils.ts";
import { Row, RowToolbar } from "../Row.tsx";
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
		<Row
			{...restProps}
			interactive={false}
			className={classes(styles.header, restProps.className)}
			onContextMenu={(event) => {
				void showNativeContextMenu(event, menuItems);
			}}
		>
			<Toolbar.Root
				aria-label="Stack actions"
				render={<RowToolbar forceVisible className={styles.toolbar} />}
			>
				<Tooltip.Root>
					<Tooltip.Trigger
						aria-label="Collapse stack branches"
						className={getRowButtonClassName({ iconOnly: true })}
						render={<Toolbar.Button focusableWhenDisabled disabled />}
					>
						<Icon name="collapse-vertical" />
					</Tooltip.Trigger>
					<Tooltip.Portal>
						<Tooltip.Positioner sideOffset={4}>
							<Tooltip.Popup render={<TooltipPopup />}>Collapse stack branches</Tooltip.Popup>
						</Tooltip.Positioner>
					</Tooltip.Portal>
				</Tooltip.Root>

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
			</Toolbar.Root>
		</Row>
	);
};
