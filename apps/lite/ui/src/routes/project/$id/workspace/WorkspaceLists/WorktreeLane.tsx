import rowStyles from "../Row.module.css";
import sectionStyles from "../Graph/Section.module.css";
import uncommittedStyles from "./UncommittedChangesRow.module.css";
import { ChangeStats } from "../ChangeStats.tsx";
import { changeFileRowItem } from "../file-row.ts";
import { FileRow } from "../FileRow.tsx";
import { FileRowTooltipRoot, type FileRowTooltipPayload } from "../FileRowTooltip.tsx";
import { getLineStats } from "../lineStats.ts";
import { CARD_GAP, LEG_GAP, TIP_GAP, type WorktreePlacement } from "../Graph/layout.ts";
import {
	addressIdentityKey,
	branchAddress,
	commitAddress,
	fileAddress,
	worktreeChangesFileParent,
	type FileParent,
} from "#ui/addresses.ts";
import { useWorktreeRemove, useWorktreeSetArchived } from "#ui/api/mutations.ts";
import {
	guiSettingsQueryOptions,
	treeChangesDiffsQueryOptions,
	worktreeChangesQueryOptions,
	worktreesListQueryOptions,
} from "#ui/api/queries.ts";
import { commitTitle } from "#ui/commit.ts";
import { classes } from "#ui/components/classes.ts";
import { GraphGap, GraphSegment } from "#ui/components/GraphSegment.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { compareFilePaths } from "#ui/file-order.ts";
import { revealInFolderLabel } from "#ui/hotkeys.ts";
import {
	nativeMenuItem,
	nativeMenuSeparator,
	showNativeContextMenu,
	showNativeMenuFromTrigger,
	type NativeMenuItem,
} from "#ui/native-menu.ts";
import { defaultSettings } from "#ui/settings.ts";
import { setCursor, useIsCursorAt } from "#ui/use-cursor.ts";
import { addressSpaceIncludes } from "#ui/workspace/address-space.ts";
import type { BranchReference, TreeChange, Worktree } from "@gitbutler/but-sdk";
import { Toolbar, Tooltip } from "@base-ui/react";
import { useQuery } from "@tanstack/react-query";
import { Fragment, useState, type ComponentProps, type FC } from "react";
import { CommitRow } from "./CommitRow.tsx";
import { useAddressSpace } from "./context.tsx";
import { ItemRow } from "./ItemRow.tsx";
import { Row, RowLabel, RowLabelContainer, RowToolbar } from "../Row.tsx";
import { getRowButtonClassName } from "../Row-utils.ts";
import { AddressC, TreeItem } from "./TreeItem.tsx";

const noChanges: Array<TreeChange> = [];
const noop = () => {};

/**
 * A worktree's uncommitted file as a row of the applied tree. Selection is
 * wired here rather than through ItemRow, since FileRow renders the Row itself.
 */
const WorktreeFileRow: FC<
	{
		projectId: string;
		fileParent: FileParent;
		change: TreeChange;
		modifiedAtMs: number | null;
		pathDisplay: "lead" | "trail";
		tooltipHandle: Tooltip.Handle<FileRowTooltipPayload>;
		behind: number;
	} & ComponentProps<typeof Row>
> = ({
	projectId,
	fileParent,
	change,
	modifiedAtMs,
	pathDisplay,
	tooltipHandle,
	behind,
	...props
}) => {
	const address = fileAddress({ parent: fileParent, path: change.path });
	const addressSpace = useAddressSpace();
	const isSelected = useIsCursorAt("applied", addressSpace, address);
	return (
		<FileRow
			{...props}
			item={changeFileRowItem({ change, dependencyCommitIds: [], path: change.path, modifiedAtMs })}
			projectId={projectId}
			fileParent={fileParent}
			branchNameByCommitId={() => undefined}
			canCheck={false}
			canUncommit={false}
			isChecked={false}
			isReviewed={false}
			checkFile={noop}
			depth={0}
			pathDisplay={pathDisplay}
			focusScope="sidebar"
			tooltipHandle={tooltipHandle}
			rail={<GraphSegment glyph="parent" status="LocalOnly" behind={behind} />}
			inert={!addressSpaceIncludes(addressSpace, address, addressIdentityKey)}
			isSelected={isSelected}
			scrollSelectedIntoView={false}
			onSelect={() => setCursor("applied", address)}
		/>
	);
};

/**
 * The worktree's uncommitted files, heading its lane the way the uncommitted
 * files card heads the trunk: the lane's rail starts here and runs down
 * through the branch and its commits.
 */
const WorktreeUncommitted: FC<{
	projectId: string;
	worktree: string;
	behind: number;
	/** The lane's rail starts here; on the trunk, the trunk runs on through it instead. */
	startsRail: boolean;
}> = ({ projectId, worktree, behind, startsRail }) => {
	const fileParent = worktreeChangesFileParent(worktree);
	const { data: worktreeChanges } = useQuery(worktreeChangesQueryOptions(projectId, worktree));
	// The cached array itself, as the diffs query keys on its identity.
	const changes = worktreeChanges?.changes ?? noChanges;
	const { data: lineStats = getLineStats([]) } = useQuery({
		...treeChangesDiffsQueryOptions({ projectId, changes, worktree }),
		select: getLineStats,
	});
	const { data: pathFirst = defaultSettings.pathFirst } = useQuery({
		...guiSettingsQueryOptions,
		select: (cfg) => cfg.pathFirst ?? defaultSettings.pathFirst,
	});
	// One per lane: rows in separate lanes can share a DOM id, and a tooltip
	// store registers one element per id.
	const [tooltipHandle] = useState(() => Tooltip.createHandle<FileRowTooltipPayload>());
	// Loaded and holding nothing, as opposed to not loaded yet: the clean wording must not flash on the way in.
	const isClean = worktreeChanges !== undefined && changes.length === 0;
	return (
		<>
			<Row interactive={false}>
				<GraphSegment
					glyph={startsRail ? "forkRight" : "joinRight"}
					status="LocalOnly"
					behind={behind}
				/>
				<RowLabelContainer>
					<RowLabel heading singleLine>
						Uncommitted files
					</RowLabel>
					{changes.length > 0 ? (
						<ChangeStats fileCount={changes.length} lineStats={lineStats} />
					) : (
						isClean && (
							<span className={classes("text-12", uncommittedStyles.caption)}>no changes</span>
						)
					)}
				</RowLabelContainer>
			</Row>
			{changes
				.toSorted((a, b) => compareFilePaths(a.path, b.path))
				.map((change) => {
					const address = fileAddress({ parent: fileParent, path: change.path });
					return (
						<TreeItem
							key={change.path}
							address={address}
							aria-label={change.path}
							render={
								<AddressC
									projectId={projectId}
									address={address}
									outline="outside"
									render={
										<WorktreeFileRow
											projectId={projectId}
											fileParent={fileParent}
											change={change}
											modifiedAtMs={worktreeChanges?.modificationTimes[change.path] ?? null}
											pathDisplay={pathFirst ? "lead" : "trail"}
											tooltipHandle={tooltipHandle}
											behind={behind}
										/>
									}
								/>
							}
						/>
					);
				})}
			<FileRowTooltipRoot handle={tooltipHandle} />
		</>
	);
};

/**
 * The branch a worktree has checked out, as a row of the applied tree: a
 * value that selects and takes commits, with the worktree's own actions on
 * its menu. It is outside the workspace, so the branch actions that rewrite
 * the workspace are not offered.
 */
const WorktreeBranchRow: FC<
	{
		projectId: string;
		worktree: string;
		refName: BranchReference;
		behind: number;
	} & ComponentProps<typeof Row>
> = ({ projectId, worktree, refName, behind, ...props }) => {
	const address = branchAddress({ branchRef: refName.fullNameBytes });
	const { data: worktreePath } = useQuery({
		...worktreesListQueryOptions(projectId),
		select: (listing) => listing.active.find((entry) => entry.name === worktree)?.path,
	});
	const { data: terminalId } = useQuery({
		...guiSettingsQueryOptions,
		select: (cfg) => (cfg.terminalId === "" ? undefined : cfg.terminalId),
	});
	const { mutate: setArchived, isPending: isArchiving } = useWorktreeSetArchived(projectId);
	const { mutate: remove, isPending: isRemoving } = useWorktreeRemove(projectId);

	const menuItems: Array<NativeMenuItem> = [
		nativeMenuItem({
			label: "Copy Branch Name",
			onSelect: () => window.lite.clipboardWriteText(refName.displayName),
		}),
		nativeMenuItem({
			label: "Copy Worktree Path",
			enabled: worktreePath !== undefined,
			onSelect: () => window.lite.clipboardWriteText(worktreePath ?? ""),
		}),
		nativeMenuSeparator,
		nativeMenuItem({
			label: "Open in Terminal",
			enabled: worktreePath !== undefined && terminalId !== undefined,
			onSelect: () => {
				if (worktreePath !== undefined && terminalId !== undefined)
					void window.lite.openInTerminal({ terminalId, path: worktreePath });
			},
		}),
		nativeMenuItem({
			label: revealInFolderLabel,
			enabled: worktreePath !== undefined,
			onSelect: () => {
				if (worktreePath !== undefined) void window.lite.showItemInFolder(worktreePath);
			},
		}),
		nativeMenuSeparator,
		nativeMenuItem({
			label: "Archive Worktree",
			enabled: !isArchiving,
			onSelect: () => setArchived({ projectId, name: worktree, archived: true }),
		}),
		nativeMenuItem({
			label: "Remove Worktree",
			enabled: !isRemoving,
			onSelect: () => remove({ projectId, name: worktree, force: false }),
		}),
	];

	return (
		<ItemRow
			{...props}
			address={address}
			scrollSelectedIntoView={false}
			onContextMenu={(event) => {
				void showNativeContextMenu(event, menuItems);
			}}
		>
			<GraphSegment glyph="joinRight" status="LocalOnly" behind={behind} />
			<RowLabelContainer>
				<RowLabel heading singleLine>
					{refName.displayName}
				</RowLabel>
			</RowLabelContainer>
			<Toolbar.Root aria-label="Worktree branch actions" render={<RowToolbar />}>
				<Toolbar.Button
					aria-label="Worktree branch menu"
					onClick={(event) => {
						void showNativeMenuFromTrigger(event.currentTarget, menuItems);
					}}
					className={getRowButtonClassName({ iconOnly: true })}
				>
					<Icon name="kebab" />
				</Toolbar.Button>
			</Toolbar.Root>
		</ItemRow>
	);
};

/**
 * A linked worktree's rows, laid out like the sidebar itself: the worktree's
 * name labels the whole, then its uncommitted files head a rail that runs
 * down through its branch and the commits only it has, with any worktree
 * resting on one of them nested above it. The rows are the sidebar's own;
 * the address space decides which of them operations may take.
 */
const WorktreeRows: FC<{
	projectId: string;
	worktree: Worktree;
	worktrees: WorktreePlacement;
	/** Columns of line running behind the rows, left of the lane's rail. */
	behind: number;
	/** The lane's rail starts at its uncommitted files; on the trunk, the trunk runs on through. */
	startsRail: boolean;
}> = ({ projectId, worktree, worktrees, behind, startsRail }) => {
	const branch =
		worktree.refName === null ? null : branchAddress({ branchRef: worktree.refName.fullNameBytes });
	return (
		<>
			<Row interactive={false}>
				<GraphSegment glyph="space" status="LocalOnly" behind={behind} />
				<RowLabelContainer>
					<RowLabel singleLine className={rowStyles.fadedText}>
						{worktree.name}
						<span className={sectionStyles.caption}>worktree</span>
					</RowLabel>
				</RowLabelContainer>
			</Row>
			<WorktreeUncommitted
				projectId={projectId}
				worktree={worktree.name}
				behind={behind}
				startsRail={startsRail}
			/>
			{worktree.refName !== null && branch !== null && (
				<TreeItem
					address={branch}
					aria-label={worktree.refName.displayName}
					render={
						<AddressC
							projectId={projectId}
							address={branch}
							outline="outside"
							render={
								<WorktreeBranchRow
									projectId={projectId}
									worktree={worktree.name}
									refName={worktree.refName}
									behind={behind}
								/>
							}
						/>
					}
				/>
			)}
			{worktree.commits.map((commit, index) => {
				const address = commitAddress({ commitId: commit.id, changeId: commit.changeId });
				const next = worktree.commits[index + 1];
				return (
					<Fragment key={commit.id}>
						{worktrees.on.get(commit.id)?.map((nested) => (
							<WorktreeLane
								key={nested.name}
								projectId={projectId}
								worktree={nested}
								worktrees={worktrees}
								behind={behind + 1}
							/>
						))}
						<TreeItem
							address={address}
							aria-label={commitTitle(commit.message) ?? "(no message)"}
							render={
								<AddressC
									projectId={projectId}
									address={address}
									outline="outside"
									render={
										<CommitRow
											commit={commit}
											projectId={projectId}
											stackId={null}
											dryRunCommit={null}
											checkCommit={noop}
											amendCommit={noop}
											canAmendCommit={false}
											below={next === undefined ? "LocalOnly" : next.state.type}
											behind={behind}
											worktree={worktree.name}
											scrollSelectedIntoView={false}
										/>
									}
								/>
							}
						/>
					</Fragment>
				);
			})}
		</>
	);
};

/**
 * A worktree resting on a shown commit: a lane a column right of that
 * commit's rail, opening above the commit and bending back onto its rail
 * in the gap below.
 */
export const WorktreeLane: FC<{
	projectId: string;
	worktree: Worktree;
	worktrees: WorktreePlacement;
	/** Columns behind the lane's rail: the rail it rests on is the last of them. */
	behind: number;
}> = ({ projectId, worktree, worktrees, behind }) => (
	<>
		<WorktreeRows
			projectId={projectId}
			worktree={worktree}
			worktrees={worktrees}
			behind={behind}
			startsRail
		/>
		<GraphGap height={LEG_GAP} bend="LocalOnly" behind={behind - 1} />
	</>
);

/**
 * A worktree resting on the tip of a card's top branch: drawn above the branch
 * row rather than off a commit, continuing the card's line the way a branch
 * stacked on top would, since its commits are what such a branch would hold.
 * A short connector carries the rail on into the branch row below.
 */
export const WorktreeOnTip: FC<{
	projectId: string;
	worktree: Worktree;
	worktrees: WorktreePlacement;
	behind: number;
	/** The card is the main line itself, so the trunk runs into the lane from above. */
	onTrunk: boolean;
}> = ({ projectId, worktree, worktrees, behind, onTrunk }) => (
	<>
		<WorktreeRows
			projectId={projectId}
			worktree={worktree}
			worktrees={worktrees}
			behind={behind}
			startsRail={!onTrunk}
		/>
		<GraphGap height={TIP_GAP} behind={behind} />
	</>
);

/**
 * A worktree resting below the workspace or on nothing shown: a card of its
 * own under the stacks, drawn like a forked stack card off the main line.
 */
export const WorktreeCard: FC<{
	projectId: string;
	worktree: Worktree;
	worktrees: WorktreePlacement;
}> = ({ projectId, worktree, worktrees }) => (
	<>
		<div className={sectionStyles.card}>
			<Row interactive={false} className={sectionStyles.air}>
				<GraphSegment glyph="space" status="LocalOnly" behind={1} />
			</Row>
			<WorktreeRows
				projectId={projectId}
				worktree={worktree}
				worktrees={worktrees}
				behind={1}
				startsRail
			/>
			<Row interactive={false} className={sectionStyles.stub}>
				<GraphSegment glyph="parent" status="LocalOnly" behind={1} />
			</Row>
		</div>
		<GraphGap height={CARD_GAP} bend="LocalOnly" />
	</>
);
