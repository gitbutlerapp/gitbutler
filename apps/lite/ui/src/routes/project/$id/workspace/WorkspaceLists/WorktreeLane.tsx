import rowStyles from "../Row.module.css";
import sectionStyles from "../Graph/Section.module.css";
import uncommittedStyles from "./UncommittedChangesRow.module.css";
import { ChangeStats } from "../ChangeStats.tsx";
import { changeFileRowItem } from "../file-row.ts";
import { FileRow } from "../FileRow.tsx";
import fileRowStyles from "../FileRow.module.css";
import { FileRowTooltipRoot, type FileRowTooltipPayload } from "../FileRowTooltip.tsx";
import { getLineStats } from "../lineStats.ts";
import { CARD_GAP, LEG_GAP, TIP_GAP, type WorktreePlacement } from "../Graph/layout.ts";
import {
	addressEquals,
	addressIdentityKey,
	branchAddress,
	commitAddress,
	fileAddress,
	worktreeChangesFileParent,
	type FileParent,
} from "#ui/addresses.ts";
import { useWorktreeRemove, useWorktreeSetArchived } from "#ui/api/mutations.ts";
import { assert } from "#ui/assert.ts";
import { decodeBytes } from "#ui/api/bytes.ts";
import {
	guiSettingsQueryOptions,
	treeChangesDiffsQueryOptions,
	worktreeChangesQueryOptions,
	worktreesListQueryOptions,
} from "#ui/api/queries.ts";
import { commitTitle } from "#ui/commit.ts";
import { classes } from "@gitbutler/ui-react/classes.ts";
import { GraphGap, GraphSegment, type GraphSegmentStatus } from "#ui/components/GraphSegment.tsx";
import { Icon } from "@gitbutler/ui-react/Icon.tsx";
import { compareFilePaths } from "#ui/file-order.ts";
import { revealInFolderLabel } from "#ui/hotkeys.ts";
import {
	nativeMenuItem,
	nativeMenuSeparator,
	showNativeContextMenu,
	showNativeMenuFromTrigger,
	type NativeMenuItem,
} from "#ui/native-menu.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { recordedPullRequest } from "#ui/api/ref-info.ts";
import { defaultSettings } from "#ui/settings.ts";
import {
	downstackPushStatusesFromSegments,
	emptyDownstackPushStatus,
	type DownstackPushStatus,
} from "#ui/segment.ts";
import { useAppSelector } from "#ui/store.ts";
import { setCursor, useIsCursorAt } from "#ui/use-cursor.ts";
import { addressSpaceIncludes } from "#ui/workspace/address-space.ts";
import type { Segment, TreeChange, Worktree } from "@gitbutler/but-sdk";
import { Toolbar, Tooltip } from "@base-ui/react";
import { useQuery } from "@tanstack/react-query";
import { Fragment, useId, useMemo, useState, type ComponentProps, type FC } from "react";
import { BranchRow, type BranchLane } from "./BranchRow.tsx";
import { CommitRow } from "./CommitRow.tsx";
import { useAddressSpace } from "./context.tsx";
import { commitGraphStatus, segmentPushStatusToGraphSegmentStatus } from "./graph-status.ts";
import { IncomingRows } from "./IncomingRows.tsx";
import { pushActivities, usePendingPushBranches, type PushActivity } from "./push-activity.ts";
import { RailedList, Row, RowLabel, RowLabelContainer, RowToolbar } from "../Row.tsx";
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
		pathDisplay: "lead" | "trail";
		tooltipHandle: Tooltip.Handle<FileRowTooltipPayload>;
	} & ComponentProps<typeof Row>
> = ({ projectId, fileParent, change, pathDisplay, tooltipHandle, ...props }) => {
	const address = fileAddress({ parent: fileParent, path: change.path });
	const addressSpace = useAddressSpace();
	const isSelected = useIsCursorAt("applied", addressSpace, address);
	return (
		<FileRow
			{...props}
			item={changeFileRowItem({ change, dependencyCommitIds: [], path: change.path })}
			projectId={projectId}
			fileParent={fileParent}
			branchNameByCommitId={() => undefined}
			canCheck={false}
			isChecked={false}
			isReviewed={false}
			checkFile={noop}
			depth={0}
			pathDisplay={pathDisplay}
			focusScope="sidebar"
			tooltipHandle={tooltipHandle}
			inert={!addressSpaceIncludes(addressSpace, address, addressIdentityKey)}
			isSelected={isSelected}
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
			{/* The lane runs beside its files rather than through their rows. */}
			<RailedList rail={<GraphSegment glyph="parent" status="LocalOnly" behind={behind} />}>
				<div className={fileRowStyles.rows}>
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
													pathDisplay={pathFirst ? "lead" : "trail"}
													tooltipHandle={tooltipHandle}
												/>
											}
										/>
									}
								/>
							);
						})}
				</div>
			</RailedList>
			<FileRowTooltipRoot handle={tooltipHandle} />
		</>
	);
};

/** The worktree's own actions: where it is on disk, and its place in the workspace. */
const useWorktreeMenuItems = (projectId: string, worktree: string): Array<NativeMenuItem> => {
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
	return [
		nativeMenuItem({
			label: "Copy Worktree Path",
			enabled: worktreePath !== undefined,
			onSelect: () => window.lite.clipboardWriteText(worktreePath ?? ""),
		}),
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
};

/** The worktree's name over its lane, with the worktree's own actions on its menu. */
const WorktreeHeaderRow: FC<{ projectId: string; worktree: string; behind: number }> = ({
	projectId,
	worktree,
	behind,
}) => {
	const menuItems = useWorktreeMenuItems(projectId, worktree);
	return (
		<Row
			interactive={false}
			onContextMenu={(event) => {
				void showNativeContextMenu(event, menuItems);
			}}
		>
			<GraphSegment glyph="space" status="LocalOnly" behind={behind} />
			<RowLabelContainer>
				<RowLabel singleLine className={rowStyles.fadedText}>
					{worktree}
					<span className={sectionStyles.caption}>worktree</span>
				</RowLabel>
			</RowLabelContainer>
			<Toolbar.Root aria-label="Worktree actions" render={<RowToolbar />}>
				<Toolbar.Button
					aria-label="Worktree menu"
					onClick={(event) => {
						void showNativeMenuFromTrigger(event.currentTarget, menuItems);
					}}
					className={getRowButtonClassName({ iconOnly: true })}
				>
					<Icon name="kebab" />
				</Toolbar.Button>
			</Toolbar.Root>
		</Row>
	);
};

const worktreeLane: BranchLane = { type: "worktree" };

/**
 * One segment of a worktree's stack: its branch, drawn and acted on like a
 * stack's, then its commits, each preceded by the lanes resting on it. A
 * folded branch hides its commits but keeps those lanes.
 */
const WorktreeSegment: FC<{
	projectId: string;
	worktree: string;
	segment: Segment;
	worktrees: WorktreePlacement;
	downstackPushStatus: DownstackPushStatus;
	pushActivity: PushActivity;
	below: ReadonlyMap<string, GraphSegmentStatus>;
	behind: number;
}> = ({
	projectId,
	worktree,
	segment,
	worktrees,
	downstackPushStatus,
	pushActivity,
	below,
	behind,
}) => {
	const descriptionId = useId();
	const { refName } = segment;
	const branch = refName === null ? null : branchAddress({ branchRef: refName.fullNameBytes });
	const isFolded = useAppSelector(
		(state) =>
			refName !== null &&
			projectSlice.selectors.selectSegmentFolded(
				state,
				projectId,
				decodeBytes(refName.fullNameBytes),
			),
	);
	const isRenaming = useAppSelector((state) => {
		const pending = projectSlice.selectors.selectPendingOperation(state, projectId);
		return (
			branch !== null && pending._tag === "InlineEdit" && addressEquals(branch, pending.address)
		);
	});
	const firstCommit = segment.commits[0];
	return (
		<>
			{refName !== null && branch !== null && (
				<TreeItem
					address={branch}
					aria-label={refName.displayName}
					aria-describedby={isRenaming ? undefined : descriptionId}
					render={<AddressC projectId={projectId} address={branch} outline="outside" />}
				>
					<BranchRow
						descriptionId={descriptionId}
						projectId={projectId}
						refName={refName}
						lane={worktreeLane}
						downstackPushStatus={downstackPushStatus}
						pushActivity={pushActivity}
						pushStatus={segment.pushStatus}
						remote={segment.remoteTrackingRefName}
						incoming={segment.commitsOnRemote.length}
						recordedPullRequest={recordedPullRequest(segment)}
						graphStatus={segmentPushStatusToGraphSegmentStatus(segment.pushStatus)}
						startsRail={false}
						commitCount={segment.commits.length}
						railBelow={firstCommit === undefined ? "LocalOnly" : commitGraphStatus(firstCommit)}
						behind={behind}
					/>
				</TreeItem>
			)}
			{refName !== null && !isFolded && segment.commitsOnRemote.length > 0 && (
				<IncomingRows projectId={projectId} segment={segment} refName={refName} behind={behind} />
			)}
			{segment.commits.map((commit) => {
				const address = commitAddress({
					commitId: commit.id,
					changeId: commit.changeId,
					worktree,
				});
				return (
					<Fragment key={commit.id}>
						{worktrees.on.get(commit.id)?.map((nested) => (
							<WorktreeLane
								key={nested.name}
								projectId={projectId}
								worktree={nested}
								worktrees={worktrees}
								behind={behind + 1}
								beneath={downstackPushStatus}
							/>
						))}
						{!isFolded && (
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
												checkCommit={noop}
												amendCommit={noop}
												canAmendCommit={false}
												below={below.get(commit.id) ?? "LocalOnly"}
												behind={behind}
												worktree={worktree}
												scrollSelectedIntoView={false}
											/>
										}
									/>
								}
							/>
						)}
					</Fragment>
				);
			})}
		</>
	);
};

/**
 * A linked worktree's rows, laid out like the sidebar itself: the worktree's
 * name labels the whole, then its uncommitted files head a rail that runs
 * down through its branches and the commits only it has, with any worktree
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
	/** What the lane rests on, which a push of any of its branches also pushes. */
	beneath: DownstackPushStatus;
}> = ({ projectId, worktree, worktrees, behind, startsRail, beneath }) => {
	// Manual memo: the compiler folds this into the props scope, where it is rebuilt
	// on every render and hands each branch row a fresh status.
	const downstackPushStatuses = useMemo(
		() => downstackPushStatusesFromSegments(worktree.segments, beneath),
		[worktree.segments, beneath],
	);
	const pendingPushBranches = usePendingPushBranches(projectId);
	const segmentPushActivities = pushActivities(worktree.segments, pendingPushBranches);
	// The rail under a commit takes the colour of the next commit, across segments.
	const commits = worktree.segments.flatMap((segment) => segment.commits);
	const below = new Map(
		commits.map((commit, index) => {
			const next = commits[index + 1];
			return [commit.id, next === undefined ? "LocalOnly" : commitGraphStatus(next)] as const;
		}),
	);
	return (
		<>
			<WorktreeHeaderRow projectId={projectId} worktree={worktree.name} behind={behind} />
			<WorktreeUncommitted
				projectId={projectId}
				worktree={worktree.name}
				behind={behind}
				startsRail={startsRail}
			/>
			{worktree.segments.map((segment, index) => (
				<WorktreeSegment
					key={
						segment.refName
							? decodeBytes(segment.refName.fullNameBytes)
							: (segment.commits[0]?.id ?? "detached")
					}
					projectId={projectId}
					worktree={worktree.name}
					segment={segment}
					worktrees={worktrees}
					downstackPushStatus={assert(downstackPushStatuses[index])}
					pushActivity={assert(segmentPushActivities[index])}
					below={below}
					behind={behind}
				/>
			))}
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
	/** What the commit the lane rests on pushes with itself. */
	beneath: DownstackPushStatus;
}> = ({ projectId, worktree, worktrees, behind, beneath }) => (
	<>
		<WorktreeRows
			projectId={projectId}
			worktree={worktree}
			worktrees={worktrees}
			behind={behind}
			startsRail
			beneath={beneath}
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
	/** What the top branch it sits on pushes with itself. */
	beneath: DownstackPushStatus;
}> = ({ projectId, worktree, worktrees, behind, beneath }) => (
	<>
		<WorktreeRows
			projectId={projectId}
			worktree={worktree}
			worktrees={worktrees}
			behind={behind}
			startsRail
			beneath={beneath}
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
				beneath={emptyDownstackPushStatus}
			/>
			<Row interactive={false} className={sectionStyles.stub}>
				<GraphSegment glyph="parent" status="LocalOnly" behind={1} />
			</Row>
		</div>
		<GraphGap height={CARD_GAP} bend="LocalOnly" />
	</>
);
