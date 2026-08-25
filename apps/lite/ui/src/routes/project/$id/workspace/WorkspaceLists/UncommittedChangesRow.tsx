import { useDiscardWorktreeChanges } from "#ui/api/mutations.ts";
import { startAbsorb, startKeyboardTransfer } from "#ui/use-cursor.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { FocusScopeKbd } from "#ui/components/FocusScopeKbd.tsx";
import { createDiffSpec } from "#ui/operations/diff-specs.ts";
import {
	nativeMenuItem,
	nativeMenuSeparator,
	showNativeContextMenu,
	showNativeMenuFromTrigger,
	type NativeMenuItem,
} from "#ui/native-menu.ts";
import {
	fileAddress,
	uncommittedChangesFileParent,
	uncommittedChangesAddress,
} from "#ui/addresses.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { getLineStats } from "#ui/routes/project/$id/workspace/lineStats.ts";
import { focusScope } from "#ui/focus-scopes.ts";
import { useAppSelector, useAppStore } from "#ui/store.ts";
import { Toolbar } from "@base-ui/react";
import type { TreeChange } from "@gitbutler/but-sdk";
import type { FC } from "react";
import { getRowButtonClassName } from "../Row-utils.ts";
import { ChangeStats } from "../ChangeStats.tsx";
import { RowToolbar, SectionHeaderRow } from "../Row.tsx";
import { useFileDisplayModeMenuItems } from "../useFileDisplayModeMenuItems.ts";
import { useQueries } from "@tanstack/react-query";
import { treeChangeDiffsQueryOptions } from "#ui/api/queries.ts";

export const UncommittedChangesRow: FC<{
	changes: Array<TreeChange>;
	headingId: string;
	projectId: string;
	onOpenFilter: () => void;
}> = ({ changes, headingId, projectId, onOpenFilter }) => {
	const lineStats = useQueries({
		queries: changes.map((change) => treeChangeDiffsQueryOptions({ projectId, change })),
		combine: (results) => getLineStats(results.map((result) => result.data)),
	});

	const address = uncommittedChangesAddress;
	const store = useAppStore();
	const noOperationPending = useAppSelector(
		(state) => projectSlice.selectors.selectPendingOperation(state, projectId)._tag === "None",
	);
	const { isPending: isDiscardWorktreeChangesPending, mutate: discardWorktreeChanges } =
		useDiscardWorktreeChanges();
	const fileDisplayModeMenuItems = useFileDisplayModeMenuItems();

	const absorb = () => {
		const checkedPaths = projectSlice.selectors.selectCheckedUncommittedFilePaths(
			store.getState(),
			projectId,
		);
		if (checkedPaths.size === 0) {
			startAbsorb({ sources: [address], sourceTarget: { type: "all" } });
			return;
		}

		startAbsorb({
			sources: Array.from(checkedPaths, (path) =>
				fileAddress({ parent: uncommittedChangesFileParent, path }),
			),
			sourceTarget: {
				type: "treeChanges",
				subject: {
					changes: changes.filter((change) => checkedPaths.has(change.path)),
					assignedStackId: null,
				},
			},
		});
	};

	const cutChanges = () => {
		startKeyboardTransfer({ sources: [address], kind: "move" });
		focusScope("sidebar");
	};

	const discardChanges = () => {
		discardWorktreeChanges({
			projectId,
			worktreeChanges: changes.map((change) => createDiffSpec(change, [])),
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
		nativeMenuSeparator,
		...fileDisplayModeMenuItems,
	];

	return (
		<SectionHeaderRow
			id={headingId}
			label="Uncommitted"
			childrenBefore={<FocusScopeKbd hotkey="1" scope="uncommitted-files" />}
			onContextMenu={(event) => {
				void showNativeContextMenu(event, menuItems);
			}}
			actions={
				noOperationPending && (
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
