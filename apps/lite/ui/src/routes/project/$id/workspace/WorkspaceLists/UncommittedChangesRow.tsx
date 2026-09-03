import { useDiscardWorktreeChanges } from "#ui/api/mutations.ts";
import { startAbsorb, startKeyboardTransfer } from "#ui/use-cursor.ts";
import { Icon } from "#ui/components/Icon.tsx";
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
import { useAppDispatch, useAppSelector, useAppStore } from "#ui/store.ts";
import { Toolbar } from "@base-ui/react";
import type { TreeChange } from "@gitbutler/but-sdk";
import type { FC } from "react";
import { getRowButtonClassName } from "../Row-utils.ts";
import { ChangeStats } from "../ChangeStats.tsx";
import { RowToolbar, SectionHeaderRow } from "../Row.tsx";
import { useFileDisplayModeMenuItems } from "../useFileDisplayModeMenuItems.ts";
import { PanelFoldToggle } from "./PanelFoldToggle.tsx";
import { useQuery } from "@tanstack/react-query";
import styles from "./UncommittedChangesRow.module.css";
import { treeChangesDiffsQueryOptions } from "#ui/api/queries.ts";

export const UncommittedChangesRow: FC<{
	changes: Array<TreeChange>;
	/**
	 * Whether the worktree is known to be clean. Distinct from an empty
	 * `changes`, which is also what a worktree that has not loaded yet looks
	 * like — the header must not flash the clean wording on the way in.
	 */
	isClean: boolean;
	headingId: string;
	projectId: string;
	onOpenFilter: () => void;
}> = ({ changes, isClean, headingId, projectId, onOpenFilter }) => {
	const { data: lineStats = getLineStats([]) } = useQuery({
		...treeChangesDiffsQueryOptions({ projectId, changes }),
		select: getLineStats,
	});

	const address = uncommittedChangesAddress;
	const store = useAppStore();
	const dispatch = useAppDispatch();
	const recentFirst = useAppSelector((state) =>
		projectSlice.selectors.selectUncommittedFilesRecentFirst(state, projectId),
	);
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
		nativeMenuSeparator,
		// Apart from the two above: those are exclusive of each other, this is
		// ordering and combines with either.
		nativeMenuItem({
			label: "Sort by Last Modified",
			checked: recentFirst,
			onSelect: () => {
				dispatch(projectSlice.actions.toggleUncommittedFilesRecentFirst({ projectId }));
			},
		}),
	];

	return (
		<SectionHeaderRow
			id={headingId}
			// With nothing to show, the header is the only line left, so it says the
			// state instead of naming a section whose contents would repeat it. The
			// name stays in the accessible text: this row labels the file tree and
			// is how the section is reached by heading, and neither should rename
			// itself every time the worktree empties.
			label={
				isClean ? (
					<>
						<span className={styles.headingName}>Uncommitted. </span>
						Nothing to commit
					</>
				) : (
					"Uncommitted"
				)
			}
			leading={<PanelFoldToggle projectId={projectId} panel="uncommitted" />}
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
			{/* A zero is not worth a badge: the title already says there is nothing
			    here, and a count that only ever reads "0" reads as a problem rather
			    than as the resting state. */}
			{changes.length > 0 && <ChangeStats fileCount={changes.length} lineStats={lineStats} />}
		</SectionHeaderRow>
	);
};
