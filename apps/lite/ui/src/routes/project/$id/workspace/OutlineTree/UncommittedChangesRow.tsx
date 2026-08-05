import { useDiscardWorktreeChanges } from "#ui/api/mutations.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { createDiffSpec } from "#ui/operations/diff-specs.ts";
import {
	nativeMenuItem,
	nativeMenuSeparator,
	showNativeContextMenu,
	showNativeMenuFromTrigger,
	type NativeMenuItem,
} from "#ui/native-menu.ts";
import { uncommittedChangesOperand, type Operand } from "#ui/operands.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { getLineStats } from "#ui/routes/project/$id/workspace/lineStats.ts";
import { focusSelectionScope } from "#ui/selection-scopes.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { Toolbar } from "@base-ui/react";
import type { AbsorptionTarget, TreeChange } from "@gitbutler/but-sdk";
import type { FC } from "react";
import { getRowButtonClassName } from "../Row-utils.ts";
import { ChangeStats } from "../ChangeStats.tsx";
import { RowToolbar, SectionHeaderRow } from "../Row.tsx";
import { useQueries } from "@tanstack/react-query";
import { treeChangeDiffsQueryOptions } from "#ui/api/queries.ts";

export const UncommittedChangesRow: FC<{
	changes: Array<TreeChange>;
	projectId: string;
	onOpenFilter: () => void;
}> = ({ changes, projectId, onOpenFilter }) => {
	const lineStats = useQueries({
		queries: changes.map((change) => treeChangeDiffsQueryOptions({ projectId, change })),
		combine: (results) => getLineStats(results.map((result) => result.data)),
	});

	const operand = uncommittedChangesOperand;
	const isDefaultMode = useAppSelector(
		(state) => projectSlice.selectors.selectOutlineModeState(state, projectId)._tag === "Default",
	);
	const { isPending: isDiscardWorktreeChangesPending, mutate: discardWorktreeChanges } =
		useDiscardWorktreeChanges();

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
				sources: [operand],
			}),
		);
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
		}),
		nativeMenuSeparator,
		nativeMenuItem({
			label: "Absorb",
			onSelect: absorb,
		}),
		nativeMenuItem({
			label: "Discard Changes",
			enabled: changes.length > 0 && !isDiscardWorktreeChangesPending,
			onSelect: discardChanges,
		}),
	];

	return (
		<SectionHeaderRow
			label="Uncommitted"
			onContextMenu={(event) => {
				void showNativeContextMenu(event, menuItems);
			}}
			actions={
				isDefaultMode && (
					<Toolbar.Root
						aria-label="Uncommitted changes actions"
						render={<RowToolbar forceVisible />}
					>
						{changes.length > 0 && (
							<Toolbar.Button
								aria-label="Filter files"
								onClick={onOpenFilter}
								className={getRowButtonClassName({ size: "regular", iconOnly: true })}
							>
								<Icon name="search" />
							</Toolbar.Button>
						)}

						<Toolbar.Button
							aria-label="Uncommitted changes menu"
							onClick={(event) => {
								void showNativeMenuFromTrigger(event.currentTarget, menuItems);
							}}
							className={getRowButtonClassName({ size: "regular", iconOnly: true })}
						>
							<Icon name="kebab" />
						</Toolbar.Button>
					</Toolbar.Root>
				)
			}
		>
			<ChangeStats fileCount={changes.length} lineStats={lineStats} />
		</SectionHeaderRow>
	);
};
