import { useDiscardWorktreeChanges } from "#ui/api/mutations.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { createDiffSpec } from "#ui/operations/diff-specs.ts";
import { keyboardTransferOperationMode } from "#ui/outline/mode.ts";
import { outlineHotkeys, selectionOperationHotkeys, toElectronAccelerator } from "#ui/hotkeys.ts";
import {
	nativeMenuItem,
	nativeMenuSeparator,
	showNativeContextMenu,
	showNativeMenuFromTrigger,
	type NativeMenuItem,
} from "#ui/native-menu.ts";
import { uncommittedChangesOperand, type Operand } from "#ui/operands.ts";
import { projectActions, selectProjectOutlineModeState } from "#ui/projects/state.ts";
import { focusSelectionScope } from "#ui/selection-scopes.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { Toolbar } from "@base-ui/react/toolbar";
import { AbsorptionTarget, TreeChange } from "@gitbutler/but-sdk";
import { FC } from "react";
import { getWorkspaceItemRowButtonClassName } from "../WorkspaceItemRow-utils.ts";
import {
	WorkspaceItemRowBubble,
	WorkspaceItemRowBubbleGroup,
	WorkspaceItemRowLabel,
	WorkspaceItemRowLabelContainer,
	WorkspaceItemRowToolbar,
} from "../WorkspaceItemRow.tsx";
import { ItemRow } from "./ItemRow.tsx";

export type LineStats = {
	linesAdded: number;
	linesRemoved: number;
};

export const UncommittedChangesRow: FC<{
	changes: Array<TreeChange>;
	lineStats: LineStats | null;
	projectId: string;
	onComposeCommitMessage: () => void;
}> = ({ changes, lineStats, projectId, onComposeCommitMessage }) => {
	const operand = uncommittedChangesOperand;
	const isDefaultMode = useAppSelector(
		(state) => selectProjectOutlineModeState(state, projectId)._tag === "Default",
	);
	const discardWorktreeChanges = useDiscardWorktreeChanges();

	const dispatch = useAppDispatch();
	const enterAbsorbMode = (source: Operand, sourceTarget: AbsorptionTarget) => {
		dispatch(projectActions.enterAbsorbMode({ projectId, source, sourceTarget }));
	};

	const absorb = () => {
		enterAbsorbMode(operand, { type: "all" });
	};

	const cutChanges = () => {
		dispatch(
			projectActions.enterTransferMode({
				projectId,
				mode: keyboardTransferOperationMode({
					source: operand,
					operationType: "into",
				}),
			}),
		);
		focusSelectionScope("outline");
	};

	const composeCommitMessage = () => {
		dispatch(projectActions.selectOutline({ projectId, selection: uncommittedChangesOperand }));
		onComposeCommitMessage();
	};

	const discardChanges = () => {
		discardWorktreeChanges.mutate({
			projectId,
			changes: changes.map((change) => createDiffSpec(change, [])),
		});
	};

	const menuItems: Array<NativeMenuItem> = [
		nativeMenuItem({
			label: "Compose Commit Message",
			accelerator: toElectronAccelerator(outlineHotkeys.composeCommitMessageFromChanges.hotkey),
			onSelect: composeCommitMessage,
			enabled: isDefaultMode,
		}),
		nativeMenuItem({
			label: "Cut Changes",
			enabled: changes.length > 0,
			onSelect: cutChanges,
			accelerator: toElectronAccelerator(selectionOperationHotkeys.cut.hotkey),
		}),
		nativeMenuSeparator,
		nativeMenuItem({
			label: "Absorb",
			accelerator: toElectronAccelerator(outlineHotkeys.absorb.hotkey),
			onSelect: absorb,
		}),
		nativeMenuItem({
			label: "Discard Changes",
			enabled: changes.length > 0 && !discardWorktreeChanges.isPending,
			onSelect: discardChanges,
		}),
	];

	return (
		<ItemRow
			projectId={projectId}
			operand={operand}
			onContextMenu={(event) => {
				void showNativeContextMenu(event, menuItems);
			}}
		>
			<WorkspaceItemRowLabelContainer>
				<WorkspaceItemRowLabel heading>
					{changes.length === 0 ? "Nothing to commit" : "Uncommitted changes"}
				</WorkspaceItemRowLabel>

				<WorkspaceItemRowBubble variant="fillGray">{changes.length}</WorkspaceItemRowBubble>

				{lineStats && (lineStats.linesAdded > 0 || lineStats.linesRemoved > 0) && (
					<WorkspaceItemRowBubbleGroup>
						{lineStats.linesAdded > 0 && (
							<WorkspaceItemRowBubble variant="safe">
								+{lineStats.linesAdded}
							</WorkspaceItemRowBubble>
						)}
						{lineStats.linesRemoved > 0 && (
							<WorkspaceItemRowBubble variant="danger">
								-{lineStats.linesRemoved}
							</WorkspaceItemRowBubble>
						)}
					</WorkspaceItemRowBubbleGroup>
				)}
			</WorkspaceItemRowLabelContainer>

			{isDefaultMode && (
				<Toolbar.Root
					aria-label="Uncommitted changes actions"
					render={<WorkspaceItemRowToolbar forceVisible />}
				>
					<Toolbar.Button
						aria-label="Uncommitted changes menu"
						onClick={(event) => {
							void showNativeMenuFromTrigger(event.currentTarget, menuItems);
						}}
						className={getWorkspaceItemRowButtonClassName({ iconOnly: true })}
					>
						<Icon name="kebab" />
					</Toolbar.Button>
				</Toolbar.Root>
			)}
		</ItemRow>
	);
};
