import { useUnapplyStack, useWorkspaceIntegrateUpstream } from "#ui/api/mutations.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { outlineHotkeys, toElectronAccelerator } from "#ui/hotkeys.ts";
import {
	nativeMenuItem,
	nativeMenuSeparator,
	showNativeContextMenu,
	showNativeMenuFromTrigger,
	type NativeMenuItem,
} from "#ui/native-menu.ts";
import { selectProjectOutlineModeState } from "#ui/projects/state.ts";
import { useAppSelector } from "#ui/store.ts";
import { stackBottomRelativeTo } from "#ui/api/stack.ts";
import { Toolbar } from "@base-ui/react/toolbar";
import { BottomUpdate, Stack } from "@gitbutler/but-sdk";
import { ComponentProps, FC } from "react";
import { getWorkspaceItemRowButtonClassName } from "../WorkspaceItemRow-utils.ts";
import {
	WorkspaceItemRow,
	WorkspaceItemRowLabelContainer,
	WorkspaceItemRowToolbar,
} from "../WorkspaceItemRow.tsx";

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
		(state) => selectProjectOutlineModeState(state, projectId)._tag === "Default",
	);

	const unapplyStackMutation = useUnapplyStack();
	const unapply = () => {
		// oxlint-disable-next-line typescript/no-non-null-assertion -- [ref:stack-id-required]
		unapplyStackMutation.mutate({ projectId, stackId: stack.id! });
	};

	const workspaceIntegrateUpstreamMutation = useWorkspaceIntegrateUpstream();
	const updateStack = () => {
		if (rebaseUpdate)
			workspaceIntegrateUpstreamMutation.mutate({
				projectId,
				updates: [rebaseUpdate],
				dryRun: false,
			});
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
			enabled: !unapplyStackMutation.isPending,
			onSelect: unapply,
		}),
	];

	return (
		<WorkspaceItemRow
			{...restProps}
			interactive={false}
			onContextMenu={(event) => {
				void showNativeContextMenu(event, menuItems);
			}}
		>
			<WorkspaceItemRowLabelContainer />

			{isDefaultMode && (
				<Toolbar.Root aria-label="Stack actions" render={<WorkspaceItemRowToolbar forceVisible />}>
					<Toolbar.Button
						aria-label="Stack menu"
						onClick={(event) => {
							void showNativeMenuFromTrigger(event.currentTarget, menuItems);
						}}
						className={getWorkspaceItemRowButtonClassName({ iconOnly: true })}
					>
						<Icon name="kebab" />
					</Toolbar.Button>
				</Toolbar.Root>
			)}
		</WorkspaceItemRow>
	);
};
