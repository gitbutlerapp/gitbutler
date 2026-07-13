import rowStyles from "../Row.module.css";
import { changesInWorktreeQueryOptions, headInfoQueryOptions } from "#ui/api/queries.ts";
import { relativeToEquals } from "#ui/api/relative-to.ts";
import { getHeadInfoIndex } from "#ui/api/ref-info.ts";
import { commitIsDiverged, commitTitle } from "#ui/commit.ts";
import {
	branchOperand,
	uncommittedChangesOperand,
	uncommittedChangesFileParent,
	commitOperand,
	fileOperand,
	operandIdentityKey,
	stackOperand,
	type Operand,
	operandEquals,
} from "#ui/operands.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { OperationSourceC } from "#ui/routes/project/$id/workspace/OperationSourceC.tsx";
import {
	ActiveOperation,
	OperationTarget,
	OperationTargetOutline,
} from "#ui/routes/project/$id/workspace/OperationTarget.tsx";
import { NavigationIndexContext } from "#ui/routes/project/$id/workspace/OutlineNavigationIndexContext.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { classes } from "#ui/components/classes.ts";
import { navigationIndexIncludes, type NavigationIndex } from "#ui/workspace/navigation-index.ts";
import { mergeProps, useRender } from "@base-ui/react";
import {
	BranchReference,
	Commit,
	RelativeTo,
	Segment,
	Stack,
	PushStatus,
	WorkspaceState,
} from "@gitbutler/but-sdk";
import { useQuery } from "@tanstack/react-query";
import { Match } from "effect";
import { ComponentProps, createContext, FC, Fragment, use, useRef } from "react";
import { Group, Panel, Separator, useDefaultLayout } from "react-resizable-panels";
import styles from "./OutlineTree.module.css";
import { Row, RowLabel, RowLabelContainer } from "../Row.tsx";
import { getOperation, useDryRunOperation } from "#ui/operations/operation.ts";
import { GraphSegment, GraphSegmentStatus } from "#ui/components/GraphSegment.tsx";
import { segmentBottomRelativeTo } from "#ui/api/stack.ts";
import { assert } from "#ui/assert.ts";
import { useMergedRefs } from "@base-ui/utils/useMergedRefs";
import { CommitRow } from "./CommitRow.tsx";
import { BranchRow } from "./BranchRow.tsx";
import { StackRow } from "./StackRow.tsx";
import { useOutlineTreeHotkeys } from "./hotkeys.ts";
import { UncommittedChangesRow } from "./UncommittedChangesRow.tsx";
import { FileRow } from "../FileRow.tsx";
import { getChangesFileRowItems, type FileRowItem } from "../file-row.ts";
import {
	canRemoveBranchReference,
	downstackPushStatusesFromSegments,
	type DownstackPushStatus,
} from "#ui/segment.ts";

const DryRunWorkspaceContext = createContext<WorkspaceState | null>(null);

const AbsorptionTargetKeysContext = createContext<ReadonlySet<string> | null>(null);

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

const OperandC: FC<
	{
		projectId: string;
		operand: Operand;
		outline: OperationTargetOutline;
	} & useRender.ComponentProps<"div">
> = ({ projectId, operand, outline, render, ...props }) => {
	const absorptionTargetKeys = assert(use(AbsorptionTargetKeysContext));
	const navigationIndex = assert(use(NavigationIndexContext));

	const activeOperation = useAppSelector((state) => {
		const outlineMode = projectSlice.selectors.selectOutlineModeState(state, projectId);
		const pendingTransferSpec = projectSlice.selectors.selectPendingTransferSpec(
			state,
			projectId,
			navigationIndex,
		);

		return Match.value(outlineMode).pipe(
			Match.tags({
				Absorb: (): ActiveOperation | null => {
					const isActive = absorptionTargetKeys.has(operandIdentityKey(operand));
					if (!isActive) return null;

					return { position: "into", tooltip: "Absorb target" };
				},
				Transfer: (): ActiveOperation | null => {
					if (!pendingTransferSpec || !operandEquals(pendingTransferSpec.target, operand))
						return null;

					return {
						position: pendingTransferSpec.position,
						tooltip: getOperation(pendingTransferSpec)?.label,
					};
				},
			}),
			Match.orElse(() => null),
		);
	});

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
						target={operand}
						activeOperation={activeOperation}
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

const CommitC: FC<{
	commit: Commit;
	projectId: string;
	stackId: string;
	isCommitTarget: boolean;
	dryRunCommit: Commit | null;
}> = ({ commit, projectId, stackId, isCommitTarget, dryRunCommit }) => {
	const operand = commitOperand({ stackId, commitId: commit.id });

	return (
		<TreeItem
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
							projectId={projectId}
							stackId={stackId}
							isCommitTarget={isCommitTarget}
							dryRunCommit={dryRunCommit}
						/>
					}
				/>
			}
		/>
	);
};

const UncommittedChanges: FC<{
	projectId: string;
}> = ({ projectId }) => {
	const { data: worktreeChanges } = useQuery(changesInWorktreeQueryOptions(projectId));
	const fileRowItems = worktreeChanges ? getChangesFileRowItems(worktreeChanges) : [];

	const operand = uncommittedChangesOperand;

	const { data: headInfo } = useQuery(headInfoQueryOptions(projectId));
	const headInfoIndex = headInfo ? getHeadInfoIndex(headInfo) : null;

	return (
		<TreeItem
			projectId={projectId}
			operand={operand}
			aria-label={`Uncommitted changes (${worktreeChanges?.changes.length ?? 0})`}
			className={styles.section}
		>
			<UncommittedChangesRow changes={worktreeChanges?.changes ?? []} projectId={projectId} />

			{(worktreeChanges?.changes.length ?? 0) === 0 ? (
				<Row interactive={false}>
					<RowLabelContainer>
						<RowLabel className={rowStyles.fadedText}>Nothing to commit</RowLabel>
					</RowLabelContainer>
				</Row>
			) : (
				// oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- Tree items need ARIA group semantics.
				<div role="group">
					{fileRowItems.map((item) => (
						<UncommittedFileRow
							key={item.path}
							item={item}
							projectId={projectId}
							branchNameByCommitId={(cid) =>
								headInfoIndex?.commitContextById(cid)?.segment.refName?.displayName
							}
						/>
					))}
				</div>
			)}
		</TreeItem>
	);
};

const UncommittedFileRow: FC<{
	item: FileRowItem;
	projectId: string;
	branchNameByCommitId: (commitId: string) => string | undefined;
}> = ({ item, projectId, branchNameByCommitId }) => {
	const operand = fileOperand({
		parent: uncommittedChangesFileParent,
		path: item.path,
	});
	const navigationIndex = assert(use(NavigationIndexContext));
	const isSelected = useAppSelector((state) =>
		projectSlice.selectors.selectIsSelectedOutline(state, projectId, navigationIndex, operand),
	);
	const dispatch = useAppDispatch();

	return (
		<TreeItem
			projectId={projectId}
			operand={operand}
			aria-label={item.path}
			render={
				<OperandC
					projectId={projectId}
					operand={operand}
					outline="outside"
					render={
						<FileRow
							item={item}
							projectId={projectId}
							fileParent={uncommittedChangesFileParent}
							branchNameByCommitId={branchNameByCommitId}
							inert={!navigationIndexIncludes(navigationIndex, operand, operandIdentityKey)}
							isSelected={isSelected}
							onSelect={() => {
								dispatch(projectSlice.actions.selectOutline({ projectId, selection: operand }));
							}}
						/>
					}
				/>
			}
		/>
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
	commitTarget: RelativeTo | null;
	canTearOffBranch: boolean;
	canRemoveBranch: boolean;
	downstackPushStatus: DownstackPushStatus;
	isTopSegment: boolean;
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
}) => {
	const operand = branchOperand({ stackId, branchRef: refName.fullNameBytes });

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
						? relativeToEquals(commitTarget, {
								type: "referenceBytes",
								subject: refName.fullNameBytes,
							})
						: false
				}
				pushStatus={segment.pushStatus}
				graphStatus={segmentPushStatusToGraphSegmentStatus(segment.pushStatus)}
				pullRequest={segment.metadata?.review.pullRequest ?? null}
				bottomRelativeTo={segmentBottomRelativeTo(segment)}
				isTopSegment={isTopSegment}
			/>

			{/* oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- Tree items need ARIA group semantics. */}
			<div role="group">
				<SegmentContent
					projectId={projectId}
					segment={segment}
					stackId={stackId}
					commitTarget={commitTarget}
				/>
			</div>
		</TreeItem>
	);
};

const EmptySegmentContent: FC<{
	segment: Segment;
	stackId: string;
}> = ({ segment, stackId }) => {
	const navigationIndex = assert(use(NavigationIndexContext));

	const refName = assert(segment.refName);
	const inert = !navigationIndexIncludes(
		navigationIndex,
		branchOperand({ stackId, branchRef: refName.fullNameBytes }),
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
	stackId: string;
	commitTarget: RelativeTo | null;
}> = ({ projectId, segment, stackId, commitTarget }) => {
	if (segment.commits.length === 0)
		return <EmptySegmentContent segment={segment} stackId={stackId} />;

	const dryRunWorkspace = use(DryRunWorkspaceContext);
	const dryRunHeadInfoIndex = dryRunWorkspace ? getHeadInfoIndex(dryRunWorkspace.headInfo) : null;

	return (
		<div>
			{segment.commits.map((commit) => {
				const dryRunCommitId = dryRunWorkspace?.replacedCommits[commit.id];
				const dryRunCommit =
					dryRunCommitId !== undefined
						? (dryRunHeadInfoIndex?.commitContextById(dryRunCommitId)?.commit ?? null)
						: null;
				return (
					<CommitC
						key={commit.id}
						commit={commit}
						projectId={projectId}
						stackId={stackId}
						isCommitTarget={
							commitTarget
								? relativeToEquals(commitTarget, { type: "commit", subject: commit.id })
								: false
						}
						dryRunCommit={dryRunCommit}
					/>
				);
			})}
		</div>
	);
};

const StackC: FC<{
	projectId: string;
	stack: Stack;
	commitTarget: RelativeTo | null;
}> = ({ projectId, stack, commitTarget }) => {
	// From Caleb:
	// > There shouldn't be a way within GitButler to end up with a stack without a
	//   StackId. Users can disrupt our matching against our metadata by playing
	//   with references, but we currently also try to patch it up at certain points
	//   so it probably isn't too common.
	// For now we'll treat this as non-nullable until we identify cases where it
	// could genuinely be null (assuming backend correctness).
	// oxlint-disable-next-line typescript/no-non-null-assertion -- [tag:stack-id-required]
	const stackId = stack.id!;
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
									/>
								) : (
									<SegmentContent
										projectId={projectId}
										segment={segment}
										stackId={stackId}
										commitTarget={commitTarget}
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
											? branchOperand({ stackId, branchRef: assert(segment.refName).fullNameBytes })
											: commitOperand({ stackId, commitId: assert(segment.commits.at(-1)).id }),
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
	commitTarget: RelativeTo | null;
}> = ({ projectId, commitTarget }) => {
	const navigationIndex = assert(use(NavigationIndexContext));
	const { data: headInfo } = useQuery(headInfoQueryOptions(projectId));
	const dryRunOperation = useAppSelector((state) => {
		const pendingTransferSpec = projectSlice.selectors.selectPendingTransferSpec(
			state,
			projectId,
			navigationIndex,
		);
		return pendingTransferSpec ? getOperation(pendingTransferSpec)?.operation : undefined;
	});

	// TODO: debounce?
	const dryRunOperationQuery = useDryRunOperation({ projectId, operation: dryRunOperation });
	const dryRunWorkspace = dryRunOperationQuery.data?.workspace ?? null;

	return (
		<DryRunWorkspaceContext value={dryRunWorkspace}>
			<div className={styles.stacks}>
				{(headInfo?.stacks.toReversed() ?? []).map((stack) => (
					<StackC key={stack.id} projectId={projectId} stack={stack} commitTarget={commitTarget} />
				))}
			</div>
		</DryRunWorkspaceContext>
	);
};

export const OutlineTree: FC<
	{
		projectId: string;
		commitTarget: RelativeTo | null;
		navigationIndex: NavigationIndex<Operand>;
		absorptionTargetKeys: ReadonlySet<string>;
	} & ComponentProps<"div">
> = ({
	projectId,
	commitTarget,
	navigationIndex,
	absorptionTargetKeys,
	ref: refProp,
	...props
}) => {
	const selection = useAppSelector((state) =>
		projectSlice.selectors.selectSelectionOutline(state, projectId, navigationIndex),
	);
	const hasCheckedCommits = useAppSelector((state) =>
		projectSlice.selectors.selectHasCheckedCommits(state, projectId),
	);

	const layoutId = `project=${projectId}:outline-tree`;
	const outlineLayout = useDefaultLayout({
		id: layoutId,
		panelIds: ["uncommitted-changes-panel", "stacks-panel"] satisfies Array<PanelId>,
	});

	const hotkeysRef = useRef<HTMLDivElement>(null);
	useOutlineTreeHotkeys({
		navigationIndex,
		projectId,
		ref: hotkeysRef,
	});

	return (
		<NavigationIndexContext value={navigationIndex}>
			<AbsorptionTargetKeysContext value={absorptionTargetKeys}>
				<Group
					{...props}
					id={layoutId}
					orientation="vertical"
					tabIndex={0}
					role="tree"
					aria-activedescendant={selection ? treeItemId(selection) : undefined}
					data-has-checked-commits={hasCheckedCommits || undefined}
					className={classes(props.className, styles.tree)}
					defaultLayout={outlineLayout.defaultLayout}
					onLayoutChanged={outlineLayout.onLayoutChanged}
					elementRef={useMergedRefs(refProp, hotkeysRef)}
				>
					<Panel
						id={"uncommitted-changes-panel" satisfies PanelId}
						className={styles.uncommittedChangesOuterPanel}
						defaultSize={200}
						minSize={120}
						groupResizeBehavior="preserve-pixel-size"
					>
						<OperandC
							projectId={projectId}
							operand={uncommittedChangesOperand}
							outline="inside"
							render={
								<div className={styles.panel}>
									<UncommittedChanges projectId={projectId} />
								</div>
							}
						/>
					</Panel>

					<Separator className={styles.resizeHandle} />

					<Panel id={"stacks-panel" satisfies PanelId} className={styles.panel} minSize={120}>
						<Stacks projectId={projectId} commitTarget={commitTarget} />
					</Panel>
				</Group>
			</AbsorptionTargetKeysContext>
		</NavigationIndexContext>
	);
};
