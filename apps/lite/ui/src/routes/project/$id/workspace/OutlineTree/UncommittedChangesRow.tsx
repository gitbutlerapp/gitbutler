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
import { focusSelectionScope } from "#ui/selection-scopes.ts";
import { useProjectStore } from "#ui/store.ts";
import { Toolbar } from "@base-ui/react/toolbar";
import { AbsorptionTarget, TreeChange, UnifiedPatch } from "@gitbutler/but-sdk";
import { FC } from "react";
import { getRowButtonClassName } from "../Row-utils.ts";
import { RowBubble, RowBubbleGroup, RowLabel, RowLabelContainer, RowToolbar } from "../Row.tsx";
import { ItemRow } from "./ItemRow.tsx";
import { useQueries } from "@tanstack/react-query";
import { treeChangeDiffsQueryOptions } from "#ui/api/queries.ts";
import { observer } from "mobx-react-lite";

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
}> = observer(({ changes, projectId }) => {
	const lineStats = useQueries({
		queries: changes.map((change) => treeChangeDiffsQueryOptions({ projectId, change })),
		combine: (results) => getLineStats(results.map((result) => result.data)),
	});

	const operand = uncommittedChangesOperand;
	const projectStore = useProjectStore(projectId);
	const isDefaultMode = projectStore.outlineMode._tag === "Default";
	const { isPending: isDiscardWorktreeChangesPending, mutate: discardWorktreeChanges } =
		useDiscardWorktreeChanges();

	const enterAbsorbMode = (source: Operand, sourceTarget: AbsorptionTarget) => {
		projectStore.enterAbsorbMode(source, sourceTarget);
	};

	const absorb = () => {
		enterAbsorbMode(operand, { type: "all" });
	};

	const cutChanges = () => {
		projectStore.enterKeyboardTransferMode(operand);
		focusSelectionScope("outline");
	};

	const discardChanges = () => {
		discardWorktreeChanges({
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
			enabled: changes.length > 0 && !isDiscardWorktreeChangesPending,
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
});
