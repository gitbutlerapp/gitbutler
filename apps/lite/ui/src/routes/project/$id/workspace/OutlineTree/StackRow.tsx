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
import { OutlineModeContext } from "#ui/WorkspaceContext.ts";
import { stackBottomRelativeTo } from "#ui/api/stack.ts";
import { BottomUpdate, Stack } from "@gitbutler/but-sdk";
import { ComponentProps, FC, use } from "react";
import { getRowButtonClassName } from "../Row-utils.ts";
import { Row, RowLabelContainer, RowToolbar } from "../Row.tsx";

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
	const { outlineMode } = use(OutlineModeContext);
	const isDefaultMode = outlineMode._tag === "Default";

	const unapplyStackMutation = useUnapplyStack();
	const unapply = () => {
		// oxlint-disable-next-line typescript/no-non-null-assertion -- [ref:stack-id-required]
		unapplyStackMutation.mutate({ projectId, stackId: stack.id! });
	};

	const workspaceIntegrateUpstreamMutation = useWorkspaceIntegrateUpstream();
	const updateStack = () => {
		if (rebaseUpdate) {
			workspaceIntegrateUpstreamMutation.mutate({
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
			enabled: !unapplyStackMutation.isPending,
			onSelect: unapply,
		}),
	];

	return (
		<Row
			{...restProps}
			interactive={false}
			onContextMenu={(event) => {
				void showNativeContextMenu(event, menuItems);
			}}
		>
			<RowLabelContainer />

			{isDefaultMode && (
				<RowToolbar aria-label="Stack actions" forceVisible role="toolbar">
					<button
						aria-label="Stack menu"
						type="button"
						onClick={(event) => {
							void showNativeMenuFromTrigger(event.currentTarget, menuItems);
						}}
						className={getRowButtonClassName({ iconOnly: true })}
					>
						<Icon name="kebab" />
					</button>
				</RowToolbar>
			)}
		</Row>
	);
};
