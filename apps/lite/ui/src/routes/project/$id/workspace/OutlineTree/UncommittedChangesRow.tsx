import { useDiscardWorktreeChanges } from "#ui/api/mutations.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { createDiffSpec } from "#ui/operations/diff-specs.ts";
import { outlineHotkeys, selectionOperationHotkeys, toElectronAccelerator } from "#ui/hotkeys.ts";
import {
	nativeMenuItem,
	nativeMenuSeparator,
	showNativeContextMenu,
	showNativeMenuFromTrigger,
	type NativeMenuItem,
} from "#ui/native-menu.ts";
import { uncommittedChangesOperand, type Operand } from "#ui/operands.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { focusSelectionScope } from "#ui/selection-scopes.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { Toolbar } from "@base-ui/react/toolbar";
import { AbsorptionTarget, TreeChange, UnifiedPatch } from "@gitbutler/but-sdk";
import { FC } from "react";
import { getRowButtonClassName } from "../Row-utils.ts";
import { RowBubble, RowBubbleGroup, RowLabel, RowLabelContainer, RowToolbar } from "../Row.tsx";
import { ItemRow } from "./ItemRow.tsx";
import { useTreeChangeDiffsBatchQuery } from "#ui/api/queries.ts";

type LineStats = {
	linesAdded: number;
	linesRemoved: number;
};

const getLineStats = (diffs: Array<UnifiedPatch | null | undefined>): LineStats => {
	const stats: LineStats = { linesAdded: 0, linesRemoved: 0 };
	for (const diff of diffs) {
		if (diff?.type !== "Patch") continue;
		stats.linesAdded += diff.subject.linesAdded;
		stats.linesRemoved += diff.subject.linesRemoved;
	}
	return stats;
};

export const UncommittedChangesRow: FC<{
	changes: Array<TreeChange>;
	projectId: string;
}> = ({ changes, projectId }) => {
	const { data: treeChangeDiffs = [] } = useTreeChangeDiffsBatchQuery({ projectId, changes });
	const lineStats = getLineStats(treeChangeDiffs);

	const operand = uncommittedChangesOperand;
	const isDefaultMode = useAppSelector(
		(state) => projectSlice.selectors.selectOutlineModeState(state, projectId)._tag === "Default",
	);
	const discardWorktreeChanges = useDiscardWorktreeChanges();

	const dispatch = useAppDispatch();
	const enterAbsorbMode = (source: Operand, sourceTarget: AbsorptionTarget) => {
		dispatch(projectSlice.actions.enterAbsorbMode({ projectId, source, sourceTarget }));
	};

	const absorb = () => {
		enterAbsorbMode(operand, { type: "all" });
	};

	const cutChanges = () => {
		dispatch(
			projectSlice.actions.enterKeyboardTransferMode({
				projectId,
				source: operand,
			}),
		);
		focusSelectionScope("outline");
	};

	const discardChanges = () => {
		discardWorktreeChanges.mutate({
			projectId,
			changes: changes.map((change) => createDiffSpec(change, [])),
		});
	};

	const menuItems: Array<NativeMenuItem> = [
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
			<RowLabelContainer>
				<RowLabel heading>Uncommitted changes</RowLabel>

				<RowBubble variant="fillGray">{changes.length}</RowBubble>

				{(lineStats.linesAdded > 0 || lineStats.linesRemoved > 0) && (
					<RowBubbleGroup>
						{lineStats.linesAdded > 0 && (
							<RowBubble variant="safe">+{lineStats.linesAdded}</RowBubble>
						)}
						{lineStats.linesRemoved > 0 && (
							<RowBubble variant="danger">-{lineStats.linesRemoved}</RowBubble>
						)}
					</RowBubbleGroup>
				)}
			</RowLabelContainer>

			{isDefaultMode && (
				<Toolbar.Root aria-label="Uncommitted changes actions" render={<RowToolbar forceVisible />}>
					<Toolbar.Button
						aria-label="Uncommitted changes menu"
						onClick={(event) => {
							void showNativeMenuFromTrigger(event.currentTarget, menuItems);
						}}
						className={getRowButtonClassName({ iconOnly: true })}
					>
						<Icon name="kebab" />
					</Toolbar.Button>
				</Toolbar.Root>
			)}
		</ItemRow>
	);
};
