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
import { Row, RowFoldToggle, RowLabel, RowLabelContainer, RowToolbar } from "../Row.tsx";
import { useFileDisplayModeMenuItems } from "../useFileDisplayModeMenuItems.ts";
import { GraphEdge } from "#ui/components/GraphSegment.tsx";
import { classes } from "#ui/components/classes.ts";
import { useQuery } from "@tanstack/react-query";
import styles from "./UncommittedChangesRow.module.css";
import { treeChangesDiffsQueryOptions } from "#ui/api/queries.ts";

/** The uncommitted files card's header: the trunk's head, as a top branch row is a card's. Not a value. */
export const UncommittedChangesRow: FC<{
	changes: Array<TreeChange>;
	/**
	 * Whether the worktree is known to be clean. Distinct from an empty
	 * `changes`, which is also what a worktree that has not loaded yet looks
	 * like — the header must not flash the clean wording on the way in.
	 */
	isClean: boolean;
	projectId: string;
	/** In the card, with its fold; or docked at the scroller's head while the card is out of view, a click scrolling to it. */
	mode:
		| {
				kind: "card";
				headingId: string;
				folded: boolean;
				onToggleFolded: () => void;
				onOpenFilter: () => void;
		  }
		| { kind: "docked"; onSelect: () => void };
	className?: string;
}> = ({ changes, isClean, projectId, mode, className }) => {
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

	const stats =
		changes.length > 0 ? (
			<ChangeStats fileCount={changes.length} lineStats={lineStats} />
		) : (
			isClean && <span className={classes("text-12", styles.caption)}>no changes</span>
		);
	if (mode.kind === "docked") {
		// oxlint-disable jsx-a11y/prefer-tag-over-role -- A row that scrolls, styled as the rows around it.
		return (
			<Row
				role="button"
				tabIndex={0}
				onSelect={mode.onSelect}
				onKeyDown={(event) => {
					if (event.key !== "Enter" && event.key !== " ") return;
					event.preventDefault();
					mode.onSelect();
				}}
				className={className}
			>
				<GraphEdge glyph="forkRight" />
				<RowLabelContainer className={styles.headerLabel}>
					<RowLabel heading singleLine>
						Uncommitted files
					</RowLabel>
					{stats}
				</RowLabelContainer>
			</Row>
		);
		// oxlint-enable jsx-a11y/prefer-tag-over-role
	}

	return (
		<Row
			interactive
			onSelect={mode.onToggleFolded}
			className={className}
			onContextMenu={(event) => {
				void showNativeContextMenu(event, menuItems);
			}}
		>
			<RowFoldToggle
				className={styles.foldToggle}
				folded={mode.folded}
				glyph={<GraphEdge glyph="forkRight" />}
				aria-label={`${mode.folded ? "Unfold" : "Fold"} uncommitted files`}
				onClick={mode.onToggleFolded}
			/>
			<RowLabelContainer className={styles.headerLabel}>
				<RowLabel id={mode.headingId} heading singleLine>
					Uncommitted files
				</RowLabel>
				{/* A zero is not worth a badge: the caption says the resting state instead. */}
				{stats}
			</RowLabelContainer>

			{noOperationPending && (
				<Toolbar.Root aria-label="Uncommitted changes actions" render={<RowToolbar forceVisible />}>
					{changes.length > 0 && !mode.folded && (
						<Toolbar.Button
							aria-label="Filter files"
							onClick={mode.onOpenFilter}
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
			)}
		</Row>
	);
};
