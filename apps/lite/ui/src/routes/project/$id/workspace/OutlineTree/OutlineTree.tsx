import rowStyles from "../Row.module.css";
import { changesInWorktreeQueryOptions, headInfoQueryOptions } from "#ui/api/queries.ts";
import { getHeadInfoIndex } from "#ui/api/ref-info.ts";
import { commitIsDiverged, commitTitle } from "#ui/commit.ts";
import {
	branchOperand,
	uncommittedChangesOperand,
	uncommittedChangesFileParent,
	commitOperand,
	operandIdentityKey,
	stackOperand,
	type Operand,
	operandEquals,
} from "#ui/operands.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { getTransferTarget } from "#ui/outline/mode.ts";
import { OperationSourceC } from "#ui/routes/project/$id/workspace/OperationSourceC.tsx";
import {
	OperationTarget as OperationTarget_,
	OperationTargetOutline,
} from "#ui/routes/project/$id/workspace/OperationTarget.tsx";
import { useOperationDropTarget } from "#ui/routes/project/$id/workspace/useOperationDropTarget.ts";
import { NavigationIndexContext } from "#ui/routes/project/$id/workspace/OutlineNavigationIndexContext.ts";
import { useAppDispatch, useAppSelector, useAppStore } from "#ui/store.ts";
import { classes } from "#ui/components/classes.ts";
import { navigationIndexIncludes, type NavigationIndex } from "#ui/workspace/navigation-index.ts";
import { mergeProps, Tooltip, useRender } from "@base-ui/react";
import type {
	BranchReference,
	Segment,
	Stack,
	PushStatus,
	WorkspaceState,
} from "@gitbutler/but-sdk";
import { useQuery, useSuspenseQuery } from "@tanstack/react-query";
import { Match } from "effect";
import { ComponentProps, createContext, FC, Fragment, use, useRef } from "react";
import { Group, Panel, Separator, useDefaultLayout } from "react-resizable-panels";
import styles from "./OutlineTree.module.css";
import { Row, RowLabel, RowLabelContainer } from "../Row.tsx";
import { getOperation, OperationType, useDryRunOperation } from "#ui/operations/operation.ts";
import { GraphSegment, GraphSegmentStatus } from "#ui/components/GraphSegment.tsx";
import { segmentBottomRelativeTo } from "#ui/api/stack.ts";
import { assert } from "#ui/assert.ts";
import { CommitRow } from "./CommitRow.tsx";
import { BranchRow } from "./BranchRow.tsx";
import { StackRow } from "./StackRow.tsx";
import { useOutlineTreeHotkeys } from "./hotkeys.ts";
import { UncommittedChangesRow } from "./UncommittedChangesRow.tsx";
import { getChangesFileRowItems } from "../file-row.ts";
import {
	canRemoveBranchReference,
	downstackPushStatusesFromSegments,
	type DownstackPushStatus,
} from "#ui/segment.ts";
import { checkedRange, navigationIndexRange } from "#ui/checking.ts";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { SelectionScope } from "#ui/selection-scopes.ts";
import { FilesTree } from "#ui/routes/project/$id/workspace/FilesTree.tsx";
import {
	CommitForm,
	type CommitTargetComboboxItem,
} from "#ui/routes/project/$id/workspace/CommitForm.tsx";
import {
	buildCommitTargetComboboxItems,
	selectCommitTargetComboboxItem,
} from "./commitTargetComboboxItems.ts";

const DryRunWorkspaceContext = createContext<WorkspaceState | null>(null);

const AbsorptionTargetChangeIdsContext = createContext<Set<string> | null>(null);

// This must be unique as to not collide with other IDs, and stable because it's
// stored in local storage.
type PanelId = "uncommitted-changes-panel" | "stacks-panel";

const treeItemId = (operand: Operand): string =>
	`outline-treeitem-${encodeURIComponent(operandIdentityKey(operand))}`;

const TreeItem: FC<
	{
		projectId: string;
		operand: Operand;
	} & useRender.ComponentProps<"div">
> = ({ projectId, operand, render, ...props }) => {
	const navigationIndex = assert(use(NavigationIndexContext));
	const isSelected = useAppSelector((state) =>
		projectSlice.selectors.selectIsSelectedOutline(state, projectId, navigationIndex, operand),
	);

	return useRender({
		render,
		defaultTagName: "div",
		props: mergeProps<"div">(props, {
			id: treeItemId(operand),
			role: "treeitem",
			"aria-selected": isSelected,
		}),
	});
};

const OperationTarget: FC<
	{
		enabled: boolean;
		operand: Operand;
		projectId: string;
		outline: OperationTargetOutline;
	} & useRender.ComponentProps<"button">
> = ({ enabled, operand, projectId, outline, render, ...props }) => {
	const dropRef = useOperationDropTarget({ enabled, target: operand, projectId });

	const absorptionTargetChangeIds = assert(use(AbsorptionTargetChangeIdsContext));
	const navigationIndex = assert(use(NavigationIndexContext));

	const { data: headInfoIndex } = useQuery({
		...headInfoQueryOptions(projectId),
		select: getHeadInfoIndex,
	});

	type ActiveOperation = { operationType: OperationType; tooltip?: string | undefined };
	const activeOperation = useAppSelector((state) => {
		const selection = projectSlice.selectors.selectSelectionOutline(
			state,
			projectId,
			navigationIndex,
		);
		const outlineMode = projectSlice.selectors.selectOutlineModeState(state, projectId);
		const detailsSelectionScope = projectSlice.selectors.selectDetailsSelectionScope(
			state,
			projectId,
		);

		return Match.value(outlineMode).pipe(
			Match.tags({
				Absorb: (): ActiveOperation | null => {
					const isActive =
						operand._tag === "Commit" && absorptionTargetChangeIds.has(operand.changeId);
					if (!isActive) return null;

					return { operationType: "into", tooltip: "Absorb target" };
				},
				Transfer: ({ value: mode }): ActiveOperation | null => {
					if (!headInfoIndex || mode.operationType === null) return null;

					const target = getTransferTarget(mode, selection, detailsSelectionScope);
					const isActive = target !== null && operandEquals(target, operand);
					if (!isActive) return null;

					return {
						operationType: mode.operationType,
						tooltip: getOperation({
							sources: mode.sources,
							target: operand,
							operationType: mode.operationType,
							headInfoIndex,
						})?.label,
					};
				},
			}),
			Match.orElse(() => null),
		);
	});

	return (
		<Tooltip.Root
			open={activeOperation?.tooltip !== undefined}
			disableHoverablePopup
			onOpenChange={(_, eventDetails) => {
				// Allow escape to bubble up from tree so it triggers the cancel
				// operation shortcut.
				if (eventDetails.reason === "escape-key") eventDetails.allowPropagation();
			}}
		>
			<Tooltip.Trigger
				{...props}
				render={
					<OperationTarget_
						ref={(el) => {
							dropRef.current = el;
						}}
						operationType={activeOperation?.operationType}
						outline={outline}
						render={render}
					/>
				}
			/>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={8} side="right">
					<Tooltip.Popup render={<TooltipPopup />}>{activeOperation?.tooltip}</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};

const OperandC: FC<
	{
		projectId: string;
		operand: Operand;
		outline: OperationTargetOutline;
	} & useRender.ComponentProps<"div">
> = ({ projectId, operand, outline, render, ...props }) => {
	const navigationIndex = assert(use(NavigationIndexContext));

	return useRender({
		render: (
			<OperationSourceC
				projectId={projectId}
				source={operand}
				outline={outline}
				render={
					<OperationTarget
						enabled={navigationIndexIncludes(navigationIndex, operand, operandIdentityKey)}
						projectId={projectId}
						operand={operand}
						outline={outline}
						render={render}
					/>
				}
			/>
		),
		defaultTagName: "div",
		props,
	});
};

const UncommittedChanges: FC<{
	navigationIndex: NavigationIndex<string>;
	commitTarget: CommitTargetComboboxItem | null;
	projectId: string;
	targetComboboxItems: Array<CommitTargetComboboxItem>;
}> = ({ navigationIndex, commitTarget, projectId, targetComboboxItems }) => {
	const dispatch = useAppDispatch();

	const { data: headInfoIndex } = useQuery({
		...headInfoQueryOptions(projectId),
		select: getHeadInfoIndex,
	});

	const { data: worktreeChanges } = useQuery(changesInWorktreeQueryOptions(projectId));
	const fileRowItems =
		worktreeChanges && headInfoIndex ? getChangesFileRowItems(worktreeChanges, headInfoIndex) : [];

	const fileSelection = useAppSelector((state) =>
		projectSlice.selectors.selectSelectionUncommittedFiles(state, projectId, navigationIndex),
	);

	return (
		<div className={styles.uncommittedChanges}>
			<UncommittedChangesRow changes={worktreeChanges?.changes ?? []} projectId={projectId} />

			<FilesTree
				className={styles.uncommittedChangesTree}
				data-selection-scope={"uncommitted-files" satisfies SelectionScope}
				onFocus={() =>
					dispatch(
						projectSlice.actions.setDetailsSelectionScope({
							projectId,
							scope: "uncommitted-files",
						}),
					)
				}
				emptyLabel="Nothing to commit"
				fileParent={uncommittedChangesFileParent}
				items={fileRowItems}
				navigationIndex={navigationIndex}
				onFileSelection={(selection) =>
					dispatch(projectSlice.actions.selectUncommittedFiles({ projectId, selection }))
				}
				projectId={projectId}
				ref={(el) => {
					// Don't steal focus if this component is mounted later on.
					if (document.activeElement !== document.body) return;

					el?.focus({ focusVisible: false });
				}}
				selection={fileSelection}
			/>

			<CommitForm
				projectId={projectId}
				commitTarget={commitTarget}
				targetComboboxItems={targetComboboxItems}
			/>
		</div>
	);
};

const segmentPushStatusToGraphSegmentStatus = (pushStatus: PushStatus): GraphSegmentStatus => {
	switch (pushStatus) {
		case "nothingToPush":
			return "LocalAndRemote";
		case "unpushedCommits":
		case "completelyUnpushed":
			return "LocalOnly";
		case "unpushedCommitsRequiringForce":
			return "Diverged";
		case "integrated":
			return "Integrated";
	}
};

const BranchSegment: FC<{
	projectId: string;
	segment: Segment;
	refName: BranchReference;
	stackId: string;
	commitTarget: Operand | null;
	canTearOffBranch: boolean;
	canRemoveBranch: boolean;
	downstackPushStatus: DownstackPushStatus;
	isTopSegment: boolean;
	checkCommit: (evt: { changeId: string; shiftKey: boolean }) => void;
}> = ({
	projectId,
	segment,
	refName,
	stackId,
	commitTarget,
	canTearOffBranch,
	canRemoveBranch,
	downstackPushStatus,
	isTopSegment,
	checkCommit,
}) => {
	const operand = branchOperand({ branchRef: refName.fullNameBytes });

	return (
		<TreeItem
			projectId={projectId}
			operand={operand}
			aria-label={refName.displayName}
			aria-expanded
			render={<OperandC projectId={projectId} operand={operand} outline="outside" />}
		>
			<BranchRow
				projectId={projectId}
				refName={refName}
				stackId={stackId}
				canTearOffBranch={canTearOffBranch}
				canRemoveBranch={canRemoveBranch}
				downstackPushStatus={downstackPushStatus}
				isCommitTarget={
					commitTarget
						? operandEquals(
								commitTarget,
								branchOperand({
									branchRef: refName.fullNameBytes,
								}),
							)
						: false
				}
				pushStatus={segment.pushStatus}
				graphStatus={segmentPushStatusToGraphSegmentStatus(segment.pushStatus)}
				bottomRelativeTo={segmentBottomRelativeTo(segment)}
				isTopSegment={isTopSegment}
			/>

			{/* oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- Tree items need ARIA group semantics. */}
			<div role="group">
				<SegmentContent
					projectId={projectId}
					segment={segment}
					commitTarget={commitTarget}
					checkCommit={checkCommit}
				/>
			</div>
		</TreeItem>
	);
};

const EmptySegmentContent: FC<{
	segment: Segment;
}> = ({ segment }) => {
	const navigationIndex = assert(use(NavigationIndexContext));

	const refName = assert(segment.refName);
	const inert = !navigationIndexIncludes(
		navigationIndex,
		branchOperand({ branchRef: refName.fullNameBytes }),
		operandIdentityKey,
	);

	return (
		<div>
			<Row interactive={false} inert={inert}>
				<GraphSegment
					glyph="parent"
					status={segmentPushStatusToGraphSegmentStatus(segment.pushStatus)}
				/>
				<RowLabelContainer>
					<RowLabel className={rowStyles.fadedText}>No commits.</RowLabel>
				</RowLabelContainer>
			</Row>
		</div>
	);
};

const SegmentContent: FC<{
	projectId: string;
	segment: Segment;
	commitTarget: Operand | null;
	checkCommit: (evt: { changeId: string; shiftKey: boolean }) => void;
}> = ({ projectId, segment, commitTarget, checkCommit }) => {
	if (segment.commits.length === 0) return <EmptySegmentContent segment={segment} />;

	const dryRunWorkspace = use(DryRunWorkspaceContext);
	const dryRunHeadInfoIndex = dryRunWorkspace ? getHeadInfoIndex(dryRunWorkspace.headInfo) : null;

	return (
		<div>
			{segment.commits.map((commit) => {
				const operand = commitOperand({ changeId: commit.changeId });
				const dryRunCommitId = dryRunWorkspace?.replacedCommits[commit.id];
				const dryRunCommit =
					dryRunCommitId !== undefined
						? (dryRunHeadInfoIndex?.commitContextById(dryRunCommitId)?.commit ?? null)
						: null;
				return (
					<TreeItem
						key={commit.changeId}
						projectId={projectId}
						operand={operand}
						aria-label={commitTitle(commit.message) ?? "(no message)"}
						render={
							<OperandC
								projectId={projectId}
								operand={operand}
								outline="outside"
								render={
									<CommitRow
										commit={commit}
										checkCommit={checkCommit}
										projectId={projectId}
										isCommitTarget={
											commitTarget
												? operandEquals(commitTarget, commitOperand({ changeId: commit.changeId }))
												: false
										}
										dryRunCommit={dryRunCommit}
									/>
								}
							/>
						}
					/>
				);
			})}
		</div>
	);
};

const StackC: FC<{
	projectId: string;
	stack: Stack;
	commitTarget: Operand | null;
	checkCommit: (evt: { changeId: string; shiftKey: boolean }) => void;
}> = ({ projectId, stack, commitTarget, checkCommit }) => {
	// From Caleb:
	// > There shouldn't be a way within GitButler to end up with a stack without a
	//   StackId. Users can disrupt our matching against our metadata by playing
	//   with references, but we currently also try to patch it up at certain points
	//   so it probably isn't too common.
	// For now we'll treat this as non-nullable until we identify cases where it
	// could genuinely be null (assuming backend correctness).
	// [tag:stack-id-required]
	const stackId = assert(stack.id);
	const operand = stackOperand({ stackId });
	const canTearOffBranch = stack.segments.length > 1;
	const downstackPushStatuses = downstackPushStatusesFromSegments(stack.segments);
	const navigationIndex = assert(use(NavigationIndexContext));

	return (
		<TreeItem
			projectId={projectId}
			operand={operand}
			aria-label="Stack"
			aria-expanded
			className={classes(styles.section, styles.stack)}
			render={<OperandC projectId={projectId} operand={operand} outline="outside" />}
		>
			<StackRow projectId={projectId} stack={stack} />

			{/* oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- Tree items need ARIA group semantics. */}
			<div role="group" className={styles.segments}>
				{stack.segments.map((segment, index) => {
					const downstackPushStatus = assert(downstackPushStatuses[index]);

					const key = segment.refName
						? JSON.stringify(segment.refName.fullNameBytes)
						: // A segment should always either have a branch reference or at
							// least one commit.
							assert(segment.commits[0]).id;

					return (
						<Fragment key={key}>
							<div className={styles.segment}>
								{segment.refName ? (
									<BranchSegment
										projectId={projectId}
										segment={segment}
										refName={segment.refName}
										stackId={stackId}
										commitTarget={commitTarget}
										canTearOffBranch={canTearOffBranch}
										canRemoveBranch={canRemoveBranchReference(stack, index)}
										downstackPushStatus={downstackPushStatus}
										isTopSegment={index === 0}
										checkCommit={checkCommit}
									/>
								) : (
									<SegmentContent
										projectId={projectId}
										segment={segment}
										commitTarget={commitTarget}
										checkCommit={checkCommit}
									/>
								)}
							</div>
							<Row
								interactive={false}
								className={styles.segmentParentItemRow}
								inert={
									!navigationIndexIncludes(
										navigationIndex,
										segment.commits.length === 0
											? branchOperand({ branchRef: assert(segment.refName).fullNameBytes })
											: commitOperand({ changeId: assert(segment.commits.at(-1)).changeId }),
										operandIdentityKey,
									)
								}
							>
								<GraphSegment
									glyph="parent"
									status={
										segment.commits.length === 0
											? segmentPushStatusToGraphSegmentStatus(segment.pushStatus)
											: commitIsDiverged(assert(segment.commits.at(-1)))
												? "Diverged"
												: assert(segment.commits.at(-1)).state.type
									}
								/>
							</Row>
						</Fragment>
					);
				})}
			</div>
		</TreeItem>
	);
};

const Stacks: FC<{
	projectId: string;
	commitTarget: Operand | null;
	checkCommit: (evt: { changeId: string; shiftKey: boolean }) => void;
}> = ({ projectId, commitTarget, checkCommit }) => {
	const navigationIndex = assert(use(NavigationIndexContext));
	const dispatch = useAppDispatch();
	const { data: headInfo } = useSuspenseQuery(headInfoQueryOptions(projectId));
	const headInfoIndex = getHeadInfoIndex(headInfo);
	const selection = useAppSelector((state) =>
		projectSlice.selectors.selectSelectionOutline(state, projectId, navigationIndex),
	);
	const dryRunOperation = useAppSelector((state) => {
		const outlineMode = projectSlice.selectors.selectOutlineModeState(state, projectId);
		const detailsSelectionScope = projectSlice.selectors.selectDetailsSelectionScope(
			state,
			projectId,
		);

		return Match.value(outlineMode).pipe(
			Match.tags({
				Transfer: ({ value: mode }) => {
					if (mode.operationType === null) return;

					const target = getTransferTarget(mode, selection, detailsSelectionScope);
					if (!target) return;

					return getOperation({
						sources: mode.sources,
						target,
						operationType: mode.operationType,
						headInfoIndex,
					})?.operation;
				},
			}),
			Match.orElse(() => undefined),
		);
	});

	// TODO: debounce?
	const { data: dryRunOperationResult } = useDryRunOperation({
		projectId,
		operation: dryRunOperation,
		headInfoIndex,
	});
	const dryRunWorkspace = dryRunOperationResult?.workspace ?? null;

	const hotkeysRef = useRef<HTMLDivElement>(null);
	useOutlineTreeHotkeys({
		navigationIndex,
		projectId,
		ref: hotkeysRef,
		checkCommit,
	});

	return (
		<DryRunWorkspaceContext value={dryRunWorkspace}>
			<div
				tabIndex={0}
				role="tree"
				aria-activedescendant={selection ? treeItemId(selection) : undefined}
				className={classes(styles.tree, styles.stacks)}
				data-selection-scope={"outline" satisfies SelectionScope}
				onFocus={() =>
					dispatch(projectSlice.actions.setDetailsSelectionScope({ projectId, scope: "outline" }))
				}
				ref={hotkeysRef}
			>
				{headInfo.stacks.toReversed().map((stack) => (
					<StackC
						key={stack.id}
						projectId={projectId}
						stack={stack}
						commitTarget={commitTarget}
						checkCommit={checkCommit}
					/>
				))}
			</div>
		</DryRunWorkspaceContext>
	);
};

export const OutlineTree: FC<
	{
		projectId: string;
		navigationIndex: NavigationIndex<Operand>;
		uncommittedFilesNavigationIndex: NavigationIndex<string>;
		absorptionTargetChangeIds: Set<string>;
	} & ComponentProps<"div">
> = ({
	projectId,
	navigationIndex,
	uncommittedFilesNavigationIndex,
	absorptionTargetChangeIds,
	...props
}) => {
	const { data: headInfo } = useQuery(headInfoQueryOptions(projectId));
	const headInfoIndex = headInfo ? getHeadInfoIndex(headInfo) : undefined;

	const commitTargetState = useAppSelector((state) =>
		projectSlice.selectors.selectCommitTarget(state, projectId),
	);
	const commitTargetComboboxItems = buildCommitTargetComboboxItems({
		headInfo,
		headInfoIndex,
		commitTargetState,
	});
	const commitTarget = selectCommitTargetComboboxItem({
		items: commitTargetComboboxItems,
		commitTargetState,
	});
	const hasCheckedCommits = useAppSelector((state) =>
		headInfoIndex
			? projectSlice.selectors.selectHasCheckedCommits(state, projectId, headInfoIndex)
			: false,
	);
	const store = useAppStore();
	const dispatch = useAppDispatch();

	const commitCheckRangeAnchor = useRef<string>(null);
	const commitCheckRangeEnd = useRef<string>(null);

	const rangeResolver = navigationIndexRange<Operand, string>({
		navigationIndex,
		getKey: (changeId) => operandIdentityKey(commitOperand({ changeId })),
		filterMap: (item) => (item._tag === "Commit" ? item.changeId : null),
	});
	const getCheckedRange = checkedRange(rangeResolver);

	const checkCommit = ({ changeId, shiftKey }: { changeId: string; shiftKey: boolean }): void => {
		if (!headInfoIndex) return;

		const checkedCommitIds = projectSlice.selectors.selectCheckedCommits(
			store.getState(),
			projectId,
			headInfoIndex,
		);
		const nextCommitRange = getCheckedRange({
			checked: checkedCommitIds,
			rangeAnchor: commitCheckRangeAnchor.current,
			rangeEnd: commitCheckRangeEnd.current,
		})({
			item: changeId,
			shiftKey,
		});

		commitCheckRangeAnchor.current = nextCommitRange.rangeAnchor;
		commitCheckRangeEnd.current = nextCommitRange.rangeEnd;
		dispatch(
			projectSlice.actions.setCheckedCommits({
				projectId,
				changeIds: Array.from(nextCommitRange.checked),
			}),
		);
	};

	const layoutId = `project=${projectId}:outline-tree`;
	const outlineLayout = useDefaultLayout({
		id: layoutId,
		panelIds: ["uncommitted-changes-panel", "stacks-panel"] satisfies Array<PanelId>,
	});

	return (
		<NavigationIndexContext value={navigationIndex}>
			<AbsorptionTargetChangeIdsContext value={absorptionTargetChangeIds}>
				<Group
					{...props}
					id={layoutId}
					orientation="vertical"
					data-has-checked-commits={hasCheckedCommits || undefined}
					className={classes(props.className, styles.tree)}
					defaultLayout={outlineLayout.defaultLayout}
					onLayoutChanged={outlineLayout.onLayoutChanged}
				>
					<Panel
						id={"uncommitted-changes-panel" satisfies PanelId}
						className={styles.uncommittedChangesOuterPanel}
						defaultSize={280}
						minSize={200}
						groupResizeBehavior="preserve-pixel-size"
					>
						<OperationSourceC
							projectId={projectId}
							source={uncommittedChangesOperand}
							outline="inside"
							render={
								<OperationTarget
									enabled
									projectId={projectId}
									operand={uncommittedChangesOperand}
									outline="inside"
									render={
										<div className={styles.panel}>
											<UncommittedChanges
												navigationIndex={uncommittedFilesNavigationIndex}
												commitTarget={commitTarget}
												projectId={projectId}
												targetComboboxItems={commitTargetComboboxItems}
											/>
										</div>
									}
								/>
							}
						/>
					</Panel>

					<Separator className={styles.resizeHandle} />

					<Panel id={"stacks-panel" satisfies PanelId} className={styles.panel} minSize={120}>
						<Stacks
							projectId={projectId}
							commitTarget={commitTarget?.operand ?? null}
							checkCommit={checkCommit}
						/>
					</Panel>
				</Group>
			</AbsorptionTargetChangeIdsContext>
		</NavigationIndexContext>
	);
};
