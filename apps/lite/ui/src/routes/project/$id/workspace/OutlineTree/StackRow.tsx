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
import { useProjectStore } from "#ui/store.ts";
import { stackBottomRelativeTo } from "#ui/api/stack.ts";
import { Toolbar } from "@base-ui/react/toolbar";
import { BottomUpdate, Stack } from "@gitbutler/but-sdk";
import { ComponentProps, FC } from "react";
import { getRowButtonClassName } from "../Row-utils.ts";
import { Row, RowLabelContainer, RowToolbar } from "../Row.tsx";
import { observer } from "mobx-react-lite";

export const StackRow: FC<
	{
		projectId: string;
		stack: Stack;
	} & Omit<ComponentProps<"div">, "onSelect">
> = observer(({ projectId, stack, ...restProps }) => {
	const relativeTo = stackBottomRelativeTo(stack);
	const rebaseUpdate: BottomUpdate | null = relativeTo
		? { kind: "rebase", selector: relativeTo }
		: null;
	const isDefaultMode = useProjectStore(projectId).outlineMode._tag === "Default";

	const { isPending: isUnapplyStackPending, mutate: unapplyStack } = useUnapplyStack();
	const unapply = () => {
		// oxlint-disable-next-line typescript/no-non-null-assertion -- [ref:stack-id-required]
		unapplyStack({ projectId, stackId: stack.id! });
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
			onContextMenu={(event) => {
				void showNativeContextMenu(event, menuItems);
			}}
		>
			<RowLabelContainer />

			{isDefaultMode && (
				<Toolbar.Root aria-label="Stack actions" render={<RowToolbar forceVisible />}>
					<Toolbar.Button
						aria-label="Stack menu"
						onClick={(event) => {
							void showNativeMenuFromTrigger(event.currentTarget, menuItems);
						}}
						className={getRowButtonClassName({ iconOnly: true })}
					>
						<Icon name="kebab" />
					</Toolbar.Button>
				</Toolbar.Root>
			)}
		</Row>
	);
});
