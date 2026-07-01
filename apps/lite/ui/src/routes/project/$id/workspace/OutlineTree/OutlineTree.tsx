import workspaceItemRowStyles from "../WorkspaceItemRow.module.css";
import uiStyles from "#ui/components/ui.module.css";
import { useCommitAmend, useCommitCreate } from "#ui/api/mutations.ts";
import {
	changesInWorktreeQueryOptions,
	headInfoQueryOptions,
	listProjectsQueryOptions,
	treeChangeDiffsQueryOptions,
} from "#ui/api/queries.ts";
import { relativeToEquals, relativeToKey } from "#ui/api/relative-to.ts";
import { getHeadInfoIndex, resolveRelativeTo, type HeadInfoIndex } from "#ui/api/ref-info.ts";
import { commitIsDiverged, commitTitle } from "#ui/commit.ts";
import { nativeMenuItem, showNativeMenuFromTrigger, type NativeMenuItem } from "#ui/native-menu.ts";
import {
	branchOperand,
	uncommittedChangesOperand,
	commitOperand,
	operandIdentityKey,
	stackOperand,
	type Operand,
} from "#ui/operands.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { focusSelectionScope, useOutlineSelection } from "#ui/selection-scopes.ts";
import {
	projectActions,
	selectProjectCommitTarget,
	selectProjectHasCheckedCommits,
	selectProjectOutlineModeState,
} from "#ui/projects/state.ts";
import { OperationSourceC } from "#ui/routes/project/$id/workspace/OperationSourceC.tsx";
import { OperationTarget } from "#ui/routes/project/$id/workspace/OperationTarget.tsx";
import { NavigationIndexContext } from "#ui/routes/project/$id/workspace/OutlineNavigationIndexContext.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { classes } from "#ui/components/classes.ts";
import { navigationIndexIncludes, type NavigationIndex } from "#ui/workspace/navigation-index.ts";
import { Button, mergeProps, Tooltip, useRender } from "@base-ui/react";
import { Combobox } from "@base-ui/react/combobox";
import {
	BranchReference,
	Commit,
	RefInfo,
	RelativeTo,
	Segment,
	Stack,
	PushStatus,
	UnifiedPatch,
	WorkspaceState,
} from "@gitbutler/but-sdk";
import { useHotkey, useHotkeys } from "@tanstack/react-hotkeys";
import { useQueries, useQuery, useSuspenseQuery } from "@tanstack/react-query";
import { useParams } from "@tanstack/react-router";
import { Match } from "effect";
import {
	ComponentProps,
	createContext,
	FC,
	Fragment,
	SubmitEventHandler,
	use,
	useRef,
	useState,
} from "react";
import styles from "./OutlineTree.module.css";
import {
	WorkspaceItemRow,
	WorkspaceItemRowLabel,
	WorkspaceItemRowLabelContainer,
} from "../WorkspaceItemRow.tsx";
import { getOperation, useDryRunOperation } from "#ui/operations/operation.ts";
import { reverse } from "effect/Array";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { GraphSegment, Status } from "#ui/components/GraphSegment.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { Kbd } from "#ui/components/Kbd.tsx";
import {
	changesHotkeys,
	formatForDisplaySorted,
	outlineHotkeys,
	toElectronAccelerator,
} from "#ui/hotkeys.ts";
import { segmentBottomRelativeTo } from "#ui/api/stack.ts";
import { assert } from "#ui/assert.ts";
import { useMergedRefs } from "@base-ui/utils/useMergedRefs";
import { OperationControls } from "#ui/routes/project/$id/workspace/OperationControls.tsx";
import { useIsSelected } from "./useIsSelected.ts";
import { CommitRow } from "./CommitRow.tsx";
import { BranchRow } from "./BranchRow.tsx";
import { StackRow } from "./StackRow.tsx";
import { useOutlineTreeHotkeys } from "./hotkeys.ts";
import { partialStackStatesFromSegments, type PartialStackState } from "./partialStackState.ts";
import { UncommittedChangesRow, type LineStats } from "./UncommittedChangesRow.tsx";

const DryRunWorkspaceContext = createContext<WorkspaceState | null>(null);

const AbsorptionTargetKeysContext = createContext<ReadonlySet<string> | null>(null);

export const OutlineTree: FC<
	{
		navigationIndex: NavigationIndex<Operand>;
		absorptionTargetKeys: ReadonlySet<string>;
	} & ComponentProps<"div">
> = ({ navigationIndex, absorptionTargetKeys, ref: refProp, ...props }) => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });

	const selection = useOutlineSelection({ projectId, navigationIndex });
	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));
	const hasCheckedCommits = useAppSelector((state) =>
		selectProjectHasCheckedCommits(state, projectId),
	);

	const dryRunOperation = Match.value(outlineMode).pipe(
		Match.tag("Transfer", ({ value: mode }) =>
			selection && mode.operationType !== null
				? getOperation({
						source: mode.source,
						target: selection,
						operationType: mode.operationType,
					})?.operation
				: undefined,
		),
		Match.orElse(() => undefined),
	);

	// TODO: debounce?
	const dryRunOperationQuery = useDryRunOperation({ projectId, operation: dryRunOperation });
	const dryRunWorkspace = dryRunOperationQuery.data?.workspace ?? null;

	const { data: headInfo } = useQuery(headInfoQueryOptions(projectId));
	const headInfoIndex = headInfo ? getHeadInfoIndex(headInfo) : undefined;

	const ref = useRef<HTMLDivElement>(null);

	useOutlineTreeHotkeys({
		navigationIndex,
		projectId,
		ref,
		onComposeCommitMessage: focusCommitMessageInput,
	});

	const commitTargetState = useAppSelector((state) => selectProjectCommitTarget(state, projectId));
	const targetComboboxItems = buildCommitTargetComboboxItems({
		headInfo,
		headInfoIndex,
		commitTargetState,
	});
	const commitTarget = selectCommitTargetComboboxItem({
		items: targetComboboxItems,
		commitTargetState,
	});

	const { data: projects } = useSuspenseQuery(listProjectsQueryOptions);
	const selectedProject = projects.find((project) => project.id === projectId);
	if (!selectedProject) throw new Error("Could not find selected project");

	return (
		<NavigationIndexContext value={navigationIndex}>
			<AbsorptionTargetKeysContext value={absorptionTargetKeys}>
				<DryRunWorkspaceContext value={dryRunWorkspace}>
					<div
						{...props}
						tabIndex={0}
						role="tree"
						aria-activedescendant={selection ? treeItemId(selection) : undefined}
						data-has-checked-commits={hasCheckedCommits || undefined}
						className={classes(props.className, styles.tree)}
						ref={useMergedRefs(refProp, ref)}
					>
						<div className={styles.uncommittedChangesContainer}>
							<UncommittedChanges
								projectId={projectId}
								commitTarget={commitTarget}
								targetComboboxItems={targetComboboxItems}
							/>
						</div>

						<div className={classes(styles.stacksScroller, uiStyles.scrollerWithSeparator)}>
							<div className={styles.stacks}>
								{reverse(headInfo?.stacks ?? []).map((stack) => (
									<StackC
										key={stack.id}
										projectId={projectId}
										stack={stack}
										commitTarget={commitTarget?.relativeTo ?? null}
									/>
								))}
							</div>
						</div>

						<OperationControls />
					</div>
				</DryRunWorkspaceContext>
			</AbsorptionTargetKeysContext>
		</NavigationIndexContext>
	);
};

const treeItemId = (operand: Operand): string =>
	`outline-treeitem-${encodeURIComponent(operandIdentityKey(operand))}`;

const TreeItem: FC<
	{
		projectId: string;
		operand: Operand;
	} & useRender.ComponentProps<"div">
> = ({ projectId, operand, render, ...props }) => {
	const isSelected = useIsSelected({ projectId, operand });

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
	} & useRender.ComponentProps<"div">
> = ({ projectId, operand, render, ...props }) => {
	const isSelected = useIsSelected({ projectId, operand });
	const absorptionTargetKeys = assert(use(AbsorptionTargetKeysContext));
	const isAbsorptionTarget = absorptionTargetKeys.has(operandIdentityKey(operand));
	const navigationIndex = assert(use(NavigationIndexContext));

	return useRender({
		render: (
			<OperationSourceC
				projectId={projectId}
				source={operand}
				render={
					<OperationTarget
						enabled={navigationIndexIncludes(navigationIndex, operand, operandIdentityKey)}
						projectId={projectId}
						target={operand}
						isSelected={isSelected}
						isAbsorptionTarget={isAbsorptionTarget}
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
					render={
						<CommitRow
							commit={commit}
							projectId={projectId}
							stackId={stackId}
							isCommitTarget={isCommitTarget}
							dryRunCommit={dryRunCommit}
							onComposeCommitHere={focusCommitMessageInput}
						/>
					}
				/>
			}
		/>
	);
};

type CommitTargetComboboxItem = {
	label: string;
	relativeTo: RelativeTo;
};

const buildCommitTargetComboboxItems = ({
	headInfo,
	headInfoIndex,
	commitTargetState,
}: {
	headInfo: RefInfo | undefined;
	headInfoIndex: HeadInfoIndex | undefined;
	commitTargetState: RelativeTo | null;
}): Array<CommitTargetComboboxItem> => {
	const commitTarget =
		commitTargetState?.type === "commit"
			? headInfoIndex?.commitContextById(commitTargetState.subject)?.commit
			: null;

	return [
		...(commitTarget
			? ([
					{
						label: `Commit: ${commitTitle(commitTarget.message) ?? "(no message)"}`,
						relativeTo: { type: "commit", subject: commitTarget.id },
					},
				] satisfies Array<CommitTargetComboboxItem>)
			: []),
		...(headInfo
			? reverse(headInfo.stacks).flatMap(
					(stack): Array<CommitTargetComboboxItem> =>
						stack.segments.flatMap((segment): Array<CommitTargetComboboxItem> => {
							const refName = segment.refName;
							if (!refName) return [];

							return [
								{
									label: refName.displayName,
									relativeTo: { type: "referenceBytes", subject: refName.fullNameBytes },
								},
							];
						}),
				)
			: []),
	];
};

const selectCommitTargetComboboxItem = ({
	items,
	commitTargetState,
}: {
	items: Array<CommitTargetComboboxItem>;
	commitTargetState: RelativeTo | null;
}): CommitTargetComboboxItem | null =>
	(commitTargetState &&
		items.find((item) => relativeToEquals(item.relativeTo, commitTargetState))) ??
	items[0] ??
	null;

const CommitTargetComboboxPopup: FC = () => (
	<Combobox.Popup className={classes(uiStyles.popup, "text-13", styles.commitTargetComboboxPopup)}>
		<Combobox.Input
			aria-label="Search targets"
			placeholder="Search targets…"
			className={styles.commitTargetComboboxInput}
		/>
		<Combobox.Empty>
			<div className={styles.commitTargetComboboxEmpty}>No targets found.</div>
		</Combobox.Empty>
		<Combobox.List className={styles.commitTargetComboboxList}>
			{(item: CommitTargetComboboxItem) => (
				<Combobox.Item
					key={relativeToKey(item.relativeTo)}
					value={item}
					className={styles.commitTargetComboboxItem}
				>
					{item.label}
				</Combobox.Item>
			)}
		</Combobox.List>
	</Combobox.Popup>
);

const commitMessageInputId = "commit-message-input";
const focusCommitMessageInput = () => {
	document.getElementById(commitMessageInputId)?.focus();
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

const CommitForm: FC<{
	projectId: string;
	commitTarget: CommitTargetComboboxItem | null;
	targetComboboxItems: Array<CommitTargetComboboxItem>;
}> = ({ projectId, commitTarget, targetComboboxItems }) => {
	const dispatch = useAppDispatch();
	const commitCreateMutation = useCommitCreate({ projectId });
	const commitAmendMutation = useCommitAmend({ projectId });

	const { data: worktreeChanges } = useQuery(changesInWorktreeQueryOptions(projectId));

	const operand = uncommittedChangesOperand;
	const commitTextareaRef = useRef<HTMLTextAreaElement | null>(null);

	const isDefaultMode = useAppSelector(
		(state) => selectProjectOutlineModeState(state, projectId)._tag === "Default",
	);

	const { data: headInfoIndex } = useQuery({
		...headInfoQueryOptions(projectId),
		select: getHeadInfoIndex,
	});
	const isCommitOrAmendPending = commitCreateMutation.isPending || commitAmendMutation.isPending;
	const canCommitOrAmendBase = isDefaultMode && commitTarget !== null && !isCommitOrAmendPending;
	const canCommit = canCommitOrAmendBase;
	const canAmend =
		canCommitOrAmendBase &&
		worktreeChanges &&
		worktreeChanges.changes.length > 0 &&
		headInfoIndex &&
		resolveRelativeTo({ headInfoIndex, relativeTo: commitTarget.relativeTo }) !== null;

	const [open, setOpen] = useState(false);

	const selectBranch = (option: CommitTargetComboboxItem | null) => {
		dispatch(
			projectActions.setCommitTarget({
				projectId,
				commitTarget: option?.relativeTo ?? null,
			}),
		);
		setOpen(false);
	};

	const selectChanges = () => {
		dispatch(projectActions.selectOutline({ projectId, selection: operand }));
	};
	const createCommit = () => {
		if (!commitTarget) return;

		commitCreateMutation.mutate(
			{
				message: commitTextareaRef.current?.value ?? "",
				relativeTo: commitTarget.relativeTo,
			},
			{
				onSuccess: (response) => {
					if (response.newCommit !== null && commitTextareaRef.current)
						commitTextareaRef.current.value = "";
				},
			},
		);
	};

	const amendCommit = () => {
		if (!commitTarget || !headInfoIndex) return;

		const commitId = resolveRelativeTo({
			headInfoIndex,
			relativeTo: commitTarget.relativeTo,
		});
		if (commitId === null) throw new Error("No commit to amend.");

		commitAmendMutation.mutate({ commitId });
	};
	const submit: SubmitEventHandler = (event) => {
		event.preventDefault();

		createCommit();
	};
	const commitMenuItems: Array<NativeMenuItem> = [
		// oxlint-disable-next-line react-hooks-js/refs -- False positive. Ref is only accessed in `onSelect` event handler.
		nativeMenuItem({
			label: "Commit",
			enabled: canCommit,
			accelerator: toElectronAccelerator(changesHotkeys.commit.hotkey),
			onSelect: createCommit,
		}),
		nativeMenuItem({
			label: "Amend Commit",
			enabled: canAmend,
			accelerator: toElectronAccelerator(changesHotkeys.amendCommit.hotkey),
			onSelect: amendCommit,
		}),
	];

	useHotkeys([
		{
			hotkey: changesHotkeys.selectCommitTarget.hotkey,
			callback: () => setOpen(true),
			options: {
				conflictBehavior: "allow",
				enabled: isDefaultMode && !isCommitOrAmendPending,
			},
		},
		{
			hotkey: changesHotkeys.commit.hotkey,
			callback: createCommit,
			options: {
				conflictBehavior: "allow",
				enabled: canCommit,
				meta: changesHotkeys.commit.meta,
			},
		},
		{
			hotkey: changesHotkeys.amendCommit.hotkey,
			callback: amendCommit,
			options: {
				conflictBehavior: "allow",
				enabled: canAmend,
				meta: changesHotkeys.amendCommit.meta,
			},
		},
	]);

	useHotkey("Escape", () => focusSelectionScope("outline"), {
		target: commitTextareaRef,
		conflictBehavior: "allow",
	});

	const commitTextareaLabel = `Compose commit message ${formatForDisplaySorted(
		outlineHotkeys.composeCommitMessage.hotkey,
	)}`;

	return (
		<form onSubmit={submit} className={styles.commitForm}>
			<textarea
				id={commitMessageInputId}
				ref={commitTextareaRef}
				aria-label={commitTextareaLabel}
				disabled={!isDefaultMode}
				readOnly={isCommitOrAmendPending}
				placeholder={commitTextareaLabel}
				className={classes("text-13", "text-body", styles.commitTextarea)}
				onFocus={selectChanges}
			/>

			<div className={styles.commitFormFooter}>
				<Combobox.Root<CommitTargetComboboxItem>
					items={targetComboboxItems}
					open={open}
					onOpenChange={setOpen}
					// Note `undefined` means uncontrolled.
					value={commitTarget ?? null}
					onValueChange={selectBranch}
					itemToStringLabel={(x) => x.label}
					itemToStringValue={(x) => relativeToKey(x.relativeTo)}
					isItemEqualToValue={(a, b) => relativeToEquals(a.relativeTo, b.relativeTo)}
					autoHighlight
					disabled={!isDefaultMode || isCommitOrAmendPending}
				>
					<Tooltip.Root>
						<Combobox.Trigger
							className={classes("text-13 text-semibold", styles.commitTargetComboboxTrigger)}
							aria-label="Select commit target"
							// We pass `disabled` here because we want to disable the button, not
							// the tooltip. Other props should be passed above.
							render={<Button focusableWhenDisabled render={<Tooltip.Trigger />} />}
						>
							<Icon name="bullseye" size={14} />
							<span className={styles.commitTargetComboboxTriggerLabel}>
								<Combobox.Value placeholder="Select commit target" />
							</span>
						</Combobox.Trigger>
						<Tooltip.Portal>
							<Tooltip.Positioner sideOffset={4}>
								<Tooltip.Popup
									render={<TooltipPopup kbd={changesHotkeys.selectCommitTarget.hotkey} />}
								>
									Select commit target
								</Tooltip.Popup>
							</Tooltip.Positioner>
						</Tooltip.Portal>
					</Tooltip.Root>
					<Combobox.Portal>
						<Combobox.Positioner align="start" sideOffset={4}>
							<CommitTargetComboboxPopup />
						</Combobox.Positioner>
					</Combobox.Portal>
				</Combobox.Root>

				{/* oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- New lint violation. */}
				<div role="group" className={styles.commitDropdownButton}>
					<Tooltip.Root>
						<Tooltip.Trigger
							className={getButtonClassName({ variant: "pop" })}
							// We pass `disabled` here because we want to disable the button, not
							// the tooltip. Other props should be passed above.
							render={<Button focusableWhenDisabled type="submit" disabled={!canCommit} />}
						>
							Commit
							<Kbd hotkey={changesHotkeys.commit.hotkey} />
						</Tooltip.Trigger>
						<Tooltip.Portal>
							<Tooltip.Positioner sideOffset={4}>
								<Tooltip.Popup render={<TooltipPopup kbd={changesHotkeys.commit.hotkey} />}>
									{changesHotkeys.commit.meta.name}
								</Tooltip.Popup>
							</Tooltip.Positioner>
						</Tooltip.Portal>
					</Tooltip.Root>
					<div aria-hidden className={styles.commitDropdownButtonSeparator} />
					<Button
						focusableWhenDisabled
						disabled={!(canAmend || canCommit)}
						aria-label="Commit options"
						className={getButtonClassName({ variant: "pop", iconOnly: true })}
						onClick={(event) => {
							void showNativeMenuFromTrigger(event.currentTarget, commitMenuItems);
						}}
					>
						<Icon name="chevron-down" />
					</Button>
				</div>
			</div>
		</form>
	);
};

const UncommittedChanges: FC<{
	projectId: string;
	commitTarget: CommitTargetComboboxItem | null;
	targetComboboxItems: Array<CommitTargetComboboxItem>;
}> = ({ projectId, commitTarget, targetComboboxItems }) => {
	const { data: worktreeChanges } = useQuery(changesInWorktreeQueryOptions(projectId));
	const treeChangeDiffs = useQueries({
		queries:
			worktreeChanges?.changes.map((change) =>
				treeChangeDiffsQueryOptions({ projectId, change }),
			) ?? [],
	});
	const lineStats = getLineStats(treeChangeDiffs.map((result) => result.data));

	const operand = uncommittedChangesOperand;

	return (
		<TreeItem
			projectId={projectId}
			operand={operand}
			aria-label={`Changes (${worktreeChanges?.changes.length ?? 0})`}
			className={classes(styles.section, styles.uncommittedChanges)}
			render={<OperandC projectId={projectId} operand={operand} />}
		>
			<UncommittedChangesRow
				changes={worktreeChanges?.changes ?? []}
				lineStats={lineStats}
				projectId={projectId}
				onComposeCommitMessage={focusCommitMessageInput}
			/>

			<CommitForm
				projectId={projectId}
				commitTarget={commitTarget}
				targetComboboxItems={targetComboboxItems}
			/>
		</TreeItem>
	);
};

const segmentPushStatusToStatus = (pushStatus: PushStatus): Status => {
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
	partialStackState: PartialStackState;
	isTopSegment: boolean;
}> = ({
	projectId,
	segment,
	refName,
	stackId,
	commitTarget,
	canTearOffBranch,
	canRemoveBranch,
	partialStackState,
	isTopSegment,
}) => {
	const operand = branchOperand({ stackId, branchRef: refName.fullNameBytes });

	return (
		<TreeItem
			projectId={projectId}
			operand={operand}
			aria-label={refName.displayName}
			aria-expanded
			render={<OperandC projectId={projectId} operand={operand} />}
		>
			<BranchRow
				projectId={projectId}
				refName={refName}
				stackId={stackId}
				canTearOffBranch={canTearOffBranch}
				canRemoveBranch={canRemoveBranch}
				partialStackState={partialStackState}
				isCommitTarget={
					commitTarget
						? relativeToEquals(commitTarget, {
								type: "referenceBytes",
								subject: refName.fullNameBytes,
							})
						: false
				}
				pushStatus={segment.pushStatus}
				graphStatus={segmentPushStatusToStatus(segment.pushStatus)}
				pullRequest={segment.metadata?.review.pullRequest ?? null}
				bottomRelativeTo={segmentBottomRelativeTo(segment)}
				isTopSegment={isTopSegment}
				onComposeCommitHere={focusCommitMessageInput}
			/>

			{/* oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- New lint violation. */}
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

const SegmentContent: FC<{
	projectId: string;
	segment: Segment;
	stackId: string;
	commitTarget: RelativeTo | null;
}> = ({ projectId, segment, stackId, commitTarget }) => {
	const navigationIndex = assert(use(NavigationIndexContext));

	if (segment.commits.length === 0) {
		const refName = assert(segment.refName);
		const inert = !navigationIndexIncludes(
			navigationIndex,
			branchOperand({ stackId, branchRef: refName.fullNameBytes }),
			operandIdentityKey,
		);

		return (
			<div>
				<WorkspaceItemRow interactive={false} inert={inert}>
					<GraphSegment glyph="parent" status={segmentPushStatusToStatus(segment.pushStatus)} />
					<WorkspaceItemRowLabelContainer>
						<WorkspaceItemRowLabel className={workspaceItemRowStyles.fadedText}>
							No commits.
						</WorkspaceItemRowLabel>
					</WorkspaceItemRowLabelContainer>
				</WorkspaceItemRow>
			</div>
		);
	}

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

	const partialStackStates = partialStackStatesFromSegments(stack.segments);
	// This should never fail because we always have at least one segment.
	const stackState = assert(partialStackStates[0]);
	const topBranchIndex = stack.segments.findIndex((segment) => segment.refName);

	const navigationIndex = assert(use(NavigationIndexContext));

	return (
		<TreeItem
			projectId={projectId}
			operand={operand}
			aria-label="Stack"
			aria-expanded
			className={classes(styles.section, styles.stack)}
			render={<OperandC projectId={projectId} operand={operand} />}
		>
			<StackRow projectId={projectId} stack={stack} />

			{/* oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- New lint violation. */}
			<div role="group" className={styles.segments}>
				{stack.segments.map((segment, index) => {
					const partialStackState = assert(partialStackStates[index]);
					const canRemoveBranch =
						segment.commits.length === 0 ||
						// We disallow deleting the top branch reference inside a stack of
						// multiple branches because (1) the backend misbehaves (2) and we
						// want to discourage users from creating branchless segments. See
						// discussion in
						// https://github.com/gitbutlerapp/gitbutler/pull/14059.
						(stackState.branchCount > 1 && index !== topBranchIndex);

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
										canRemoveBranch={canRemoveBranch}
										partialStackState={partialStackState}
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
							<WorkspaceItemRow
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
											? segmentPushStatusToStatus(segment.pushStatus)
											: commitIsDiverged(assert(segment.commits.at(-1)))
												? "Diverged"
												: assert(segment.commits.at(-1)).state.type
									}
								/>
							</WorkspaceItemRow>
						</Fragment>
					);
				})}
			</div>
		</TreeItem>
	);
};
