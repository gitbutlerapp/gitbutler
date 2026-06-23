import workspaceItemRowStyles from "./WorkspaceItemRow.module.css";
import uiStyles from "#ui/components/ui.module.css";
import {
	useBranchCreate,
	useCommitAmend,
	useCommitCreate,
	useCommitDiscard,
	useDiscardWorktreeChanges,
	useCommitInsertBlank,
	useCommitMove,
	useCommitReword,
	useCommitUncommit,
	usePushStack,
	useWorkspaceIntegrateUpstream,
	useRemoveBranch,
	useTearOffBranch,
	useUnapplyStack,
	useUpdateBranchName,
} from "#ui/api/mutations.ts";
import {
	changesInWorktreeQueryOptions,
	headInfoQueryOptions,
	listProjectsQueryOptions,
	treeChangeDiffsQueryOptions,
} from "#ui/api/queries.ts";
import { findBranchOperandByRef, findCommit, resolveRelativeTo } from "#ui/api/ref-info.ts";
import { decodeBytes, bytesEqual } from "#ui/api/bytes.ts";
import { commitBody, commitIsDiverged, commitTitle } from "#ui/commit.ts";
import {
	nativeMenuItem,
	nativeMenuSeparator,
	showNativeContextMenu,
	showNativeMenuFromTrigger,
	type NativeMenuItem,
} from "#ui/native-menu.ts";
import {
	branchOperand,
	changesSectionOperand,
	commitOperand,
	operandEquals,
	operandIdentityKey,
	stackOperand,
	type BranchOperand,
	type CommitOperand,
	type Operand,
} from "#ui/operands.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { keyboardTransferOperationMode } from "#ui/outline/mode.ts";
import {
	focusSelectionScope,
	resolveNavigationIndexSelection,
	useNavigationIndexHotkeys,
	useOutlineSelection,
} from "#ui/selection-scopes.ts";
import {
	projectActions,
	selectProjectCommitChecked,
	selectProjectCommitTarget,
	selectProjectHasCheckedCommits,
	selectProjectHighlightedCommitIds,
	selectProjectOutlineModeState,
	selectProjectSelectionOutline,
} from "#ui/projects/state.ts";
import { rewrittenCommitSelection } from "#ui/projects/workspace/state.ts";
import { OperationSourceC } from "#ui/routes/project/$id/workspace/OperationSourceC.tsx";
import { OperationTarget } from "#ui/routes/project/$id/workspace/OperationTarget.tsx";
import { NavigationIndexContext } from "#ui/routes/project/$id/workspace/OutlineNavigationIndexContext.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { classes } from "#ui/components/classes.ts";
import { navigationIndexIncludes, type NavigationIndex } from "#ui/workspace/navigation-index.ts";
import { Button, mergeProps, Toast, Tooltip, useRender } from "@base-ui/react";
import { Combobox } from "@base-ui/react/combobox";
import { Toolbar } from "@base-ui/react/toolbar";
import {
	AbsorptionTarget,
	BranchReference,
	Commit,
	RefInfo,
	RelativeTo,
	Segment,
	Stack,
	PushStatus,
	TreeChange,
	UnifiedPatch,
	WorkspaceState,
	InsertSide,
	BottomUpdate,
} from "@gitbutler/but-sdk";
import {
	formatForDisplay,
	useHotkey,
	UseHotkeyDefinition,
	useHotkeys,
} from "@tanstack/react-hotkeys";
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
	useId,
	useOptimistic,
	useRef,
	useState,
	useTransition,
} from "react";
import styles from "./OutlineTree.module.css";
import { Checkbox } from "#ui/components/Checkbox.tsx";
import {
	WorkspaceItemRow,
	WorkspaceItemRowLabel,
	WorkspaceItemRowLabelContainer,
	WorkspaceItemRowToolbar,
} from "./WorkspaceItemRow.tsx";
import { getWorkspaceItemRowButtonClassName } from "./WorkspaceItemRow-utils.ts";
import { getOperation, useDryRunOperation } from "#ui/operations/operation.ts";
import { createDiffSpec } from "#ui/operations/diff-specs.ts";
import { initNonEmpty, reverse, scanRight } from "effect/Array";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { GraphSegment, Status } from "#ui/components/GraphSegment.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { Kbd } from "#ui/components/Kbd.tsx";
import {
	changesHotkeys,
	outlineHotkeys,
	selectionOperationHotkeys,
	toElectronAccelerator,
	type CommandGroup,
} from "#ui/hotkeys.ts";
import { segmentBottomRelativeTo, stackBottomRelativeTo } from "#ui/api/stack.ts";
import { assert } from "#ui/assert.ts";
import { errorMessageForToast } from "#ui/errors.ts";
import { useMergedRefs } from "@base-ui/utils/useMergedRefs";
import { OperationControls } from "#ui/routes/project/$id/workspace/OperationControls.tsx";

const DryRunWorkspaceContext = createContext<WorkspaceState | null>(null);

const AbsorptionTargetKeysContext = createContext<ReadonlySet<string> | null>(null);

const isCommitDiscardBoundary = (operand: Operand): boolean =>
	operand._tag === "Branch" || operand._tag === "ChangesSection";

const selectAfterDiscardedCommit = ({
	navigationIndex,
	commit,
}: {
	navigationIndex: NavigationIndex<Operand>;
	commit: CommitOperand;
}): Operand | null => {
	const commitIndex = navigationIndex.indexByKey.get(operandIdentityKey(commitOperand(commit)));
	if (commitIndex === undefined) return null;

	for (const item of navigationIndex.items.slice(commitIndex + 1)) {
		if (isCommitDiscardBoundary(item)) break;
		if (item._tag === "Commit") return item;
	}

	for (const item of navigationIndex.items.slice(0, commitIndex).reverse()) {
		if (isCommitDiscardBoundary(item)) break;
		if (item._tag === "Commit") return item;
	}

	for (const item of navigationIndex.items.slice(0, commitIndex + 1).reverse()) {
		if (item._tag === "Branch") return item;
		if (isCommitDiscardBoundary(item)) break;
	}

	return null;
};

const useOutlineTreeHotkeys = ({
	navigationIndex,
	projectId,
	ref,
}: {
	navigationIndex: NavigationIndex<Operand>;
	projectId: string;
	ref: React.RefObject<HTMLElement | null>;
}) => {
	const { data: headInfo } = useQuery(headInfoQueryOptions(projectId));
	const selection = useOutlineSelection({ projectId, navigationIndex });
	const isDefaultMode = useAppSelector(
		(state) => selectProjectOutlineModeState(state, projectId)._tag === "Default",
	);

	const selectedStack =
		selection && "stackId" in selection
			? headInfo?.stacks.find((stack) => stack.id === selection.stackId)
			: undefined;
	const selectedBranchSegment =
		selection?._tag === "Branch"
			? selectedStack?.segments.find(
					(segment) =>
						!!segment.refName && bytesEqual(segment.refName.fullNameBytes, selection.branchRef),
				)
			: undefined;

	const selectedBranchCommitsChecked = useAppSelector((state) =>
		selectedBranchSegment && selectedBranchSegment.commits.length > 0
			? selectedBranchSegment.commits.every((commit) =>
					selectProjectCommitChecked(state, projectId, commit.id),
				)
			: false,
	);
	const selectedCommitChecked = useAppSelector((state) =>
		selection?._tag === "Commit"
			? selectProjectCommitChecked(state, projectId, selection.commitId)
			: false,
	);

	const dispatch = useAppDispatch();

	const commitMoveMutation = useCommitMove();
	const commitDiscardMutation = useCommitDiscard();
	const commitInsertBlankMutation = useCommitInsertBlank();
	const commitAmendMutation = useCommitAmend({ projectId });
	const pushStackMutation = usePushStack();
	const workspaceIntegrateUpstreamMutation = useWorkspaceIntegrateUpstream();
	const branchCreateMutation = useBranchCreate();

	const openBranchPicker = () => {
		dispatch(projectActions.openBranchPicker({ projectId }));
	};

	const enterAbsorbMode = (source: Operand, sourceTarget: AbsorptionTarget) => {
		dispatch(projectActions.enterAbsorbMode({ projectId, source, sourceTarget }));
	};

	const amendCommit = () => {
		if (selection?._tag !== "Commit") return;

		commitAmendMutation.mutate({ commitId: selection.commitId });
	};

	const setCommitTarget = (relativeTo: RelativeTo) => {
		dispatch(projectActions.setCommitTarget({ projectId, commitTarget: relativeTo }));
	};

	const composeCommitHere = (relativeTo: RelativeTo) => {
		setCommitTarget(relativeTo);
		focusCommitMessageInput();
	};

	const insertEmptyCommit = () => {
		if (!selection) return;

		type Placement = { relativeTo: RelativeTo; side: InsertSide };
		const placement = Match.value(selection).pipe(
			Match.tags({
				Commit: (selection): Placement => ({
					relativeTo: { type: "commit", subject: selection.commitId },
					side: "above",
				}),
				Branch: (selection): Placement => ({
					relativeTo: {
						type: "referenceBytes",
						subject: selection.branchRef,
					},
					side: "below",
				}),
			}),
			Match.orElse(() => null),
		);

		if (!placement) return;

		commitInsertBlankMutation.mutate({
			projectId,
			relativeTo: placement.relativeTo,
			side: placement.side,
			dryRun: false,
		});
	};

	const createDependentBranchAbove = (relativeTo: RelativeTo) => {
		branchCreateMutation.mutate(
			{
				projectId,
				newRef: null,
				placement: {
					type: "dependent",
					subject: {
						relativeTo,
						side: "above",
					},
				},
			},
			{
				onSuccess: (response) => {
					const newBranch = findBranchOperandByRef({
						headInfo: response.workspace.headInfo,
						branchRef: response.newRef.fullNameBytes,
					});
					if (newBranch)
						dispatch(
							projectActions.selectOutline({
								projectId,
								selection: branchOperand(newBranch),
							}),
						);
				},
			},
		);
	};

	const toggleSelectedCommitChecked = () => {
		if (!selection || selection._tag !== "Commit") return;

		dispatch(
			projectActions.setCommitChecked({
				projectId,
				commitId: selection.commitId,
				checked: !selectedCommitChecked,
			}),
		);
	};

	const toggleSelectedBranchChecked = () => {
		if (!selectedBranchSegment) return;

		dispatch(
			projectActions.setCommitsChecked({
				projectId,
				commitIds: selectedBranchSegment.commits.map((commit) => commit.id),
				checked: !selectedBranchCommitsChecked,
			}),
		);
	};

	const moveSelectedCommit = (offset: -1 | 1) => {
		if (!selection || selection._tag !== "Commit") return;

		const source = commitOperand(selection);
		const selectionIdx = navigationIndex.indexByKey.get(operandIdentityKey(source));
		if (selectionIdx === undefined) return;

		const nextItem = navigationIndex.items[selectionIdx + offset];
		if (!nextItem) return;

		const relativeTo = Match.value(nextItem).pipe(
			Match.tags({
				Commit: ({ commitId }): RelativeTo => ({ type: "commit", subject: commitId }),
				Branch: ({ branchRef }): RelativeTo => ({
					type: "referenceBytes",
					subject: branchRef,
				}),
			}),
			Match.orElse(() => null),
		);
		if (!relativeTo) return;

		commitMoveMutation.mutate({
			projectId,
			subjectCommitIds: [selection.commitId],
			relativeTo,
			side: offset === -1 ? "above" : "below",
			dryRun: false,
		});
	};

	const deleteSelectedCommit = () => {
		if (!selection || selection._tag !== "Commit") return;

		const selectionAfterDiscard = selectAfterDiscardedCommit({
			navigationIndex,
			commit: { stackId: selection.stackId, commitId: selection.commitId },
		});

		commitDiscardMutation.mutate(
			{
				projectId,
				subjectCommitId: selection.commitId,
				dryRun: false,
			},
			{
				onSuccess: (response) => {
					dispatch(
						projectActions.selectOutline({
							projectId,
							selection: rewrittenCommitSelection({
								selection: selectionAfterDiscard,
								replacedCommits: response.workspace.replacedCommits,
								headInfo: response.workspace.headInfo,
							}),
						}),
					);
				},
			},
		);
	};

	const selectedPushContext = Match.value(selection).pipe(
		Match.tags({
			Branch: (selection) => {
				if (!selectedStack || selectedStack.id === null) return null;

				const segmentIndex = selectedStack.segments.findIndex(
					(segment) =>
						!!segment.refName && bytesEqual(segment.refName.fullNameBytes, selection.branchRef),
				);
				if (segmentIndex === -1) return null;

				return pushContextForSegment({ segments: selectedStack.segments, segmentIndex });
			},
			Commit: (selection) => {
				if (!selectedStack || selectedStack.id === null) return null;

				const segmentIndex = selectedStack.segments.findIndex((segment) =>
					segment.commits.some((commit) => commit.id === selection.commitId),
				);
				if (segmentIndex === -1) return null;

				return pushContextForSegment({ segments: selectedStack.segments, segmentIndex });
			},
		}),
		Match.orElse(() => null),
	);
	const selectedStackRelativeTo = selectedStack ? stackBottomRelativeTo(selectedStack) : null;
	const selectedStackRebaseUpdate: BottomUpdate | null = selectedStackRelativeTo
		? { kind: "rebase", selector: selectedStackRelativeTo }
		: null;

	const pushSelectedBranch = () => {
		if (!selectedPushContext) return;

		const partialStackState = partialStackStateFromSegments(
			selectedPushContext.partialStackSegments,
		);

		pushStackMutation.mutate({
			projectId,
			branch: decodeBytes(selectedPushContext.refName.fullNameBytes),
			withForce: partialStackState.pushWithForce,
			skipForcePushProtection: false,
			runHooks: true,
			pushOpts: [],
		});
	};

	const updateSelectedStack = () => {
		if (selectedStackRebaseUpdate)
			workspaceIntegrateUpstreamMutation.mutate({
				projectId,
				updates: [selectedStackRebaseUpdate],
				dryRun: false,
			});
	};

	const defaultOutlineHotkeysEnabled = isDefaultMode;
	const isSelectedCommit = selection?._tag === "Commit";
	const isSelectedBranch = selection?._tag === "Branch";
	const isSelectedChanges = selection?._tag === "ChangesSection";
	const canPushSelectedBranch =
		!!selectedPushContext &&
		!pushStackMutation.isPending &&
		partialStackPushDisabledReason(
			partialStackStateFromSegments(selectedPushContext.partialStackSegments),
		) === null;

	useNavigationIndexHotkeys({
		ref,
		navigationIndex,
		projectId,
		group: "Outline",
		selectionScope: "outline",
		select: (newItem) => dispatch(projectActions.selectOutline({ projectId, selection: newItem })),
		selection,
		getKey: operandIdentityKey,
		operationSourceForItem: (operand) => operand,
		selectSectionPredicate: (operand) =>
			operand._tag === "Branch" || operand._tag === "ChangesSection",
	});

	useHotkeys([
		{
			hotkey: outlineHotkeys.selectBranch.hotkey,
			callback: openBranchPicker,
			options: {
				conflictBehavior: "allow",
				meta: outlineHotkeys.selectBranch.meta,
			},
		},
		{
			hotkey: outlineHotkeys.selectChanges.hotkey,
			callback: () => {
				dispatch(projectActions.selectOutline({ projectId, selection: changesSectionOperand }));
				focusSelectionScope("outline");
			},
			options: { conflictBehavior: "allow", meta: outlineHotkeys.selectChanges.meta },
		},
		{
			hotkey: outlineHotkeys.composeCommitMessage.hotkey,
			callback: () => {
				dispatch(projectActions.selectOutline({ projectId, selection: changesSectionOperand }));
				focusCommitMessageInput();
			},
			options: {
				conflictBehavior: "allow",
				meta: outlineHotkeys.composeCommitMessage.meta,
			},
		},
		...Match.value(selection).pipe(
			Match.tag(
				"Commit",
				(selection): UseHotkeyDefinition => ({
					hotkey: outlineHotkeys.rewordCommit.hotkey,
					callback: () => {
						dispatch(projectActions.startRewordCommit({ projectId, commit: selection }));
					},
					options: {
						conflictBehavior: "allow",
						enabled: defaultOutlineHotkeysEnabled,
						target: ref,
						meta: outlineHotkeys.rewordCommit.meta,
					},
				}),
			),
			Match.tag(
				"Branch",
				(selection): UseHotkeyDefinition => ({
					hotkey: outlineHotkeys.renameBranch.hotkey,
					callback: () => {
						dispatch(projectActions.startRenameBranch({ projectId, branch: selection }));
					},
					options: {
						conflictBehavior: "allow",
						enabled: defaultOutlineHotkeysEnabled,
						target: ref,
						meta: outlineHotkeys.renameBranch.meta,
					},
				}),
			),
			Match.tag(
				"ChangesSection",
				(): UseHotkeyDefinition => ({
					hotkey: outlineHotkeys.composeCommitMessageFromChanges.hotkey,
					callback: focusCommitMessageInput,
					options: {
						conflictBehavior: "allow",
						enabled: defaultOutlineHotkeysEnabled,
						target: ref,
						meta: outlineHotkeys.composeCommitMessageFromChanges.meta,
					},
				}),
			),
			Match.orElse(() => null),
			(x) => (x ? [x] : []),
		),
		{
			hotkey: outlineHotkeys.amendCommit.hotkey,
			callback: amendCommit,
			options: {
				conflictBehavior: "allow",
				enabled: defaultOutlineHotkeysEnabled && isSelectedCommit && !commitAmendMutation.isPending,
				target: ref,
				meta: outlineHotkeys.amendCommit.meta,
			},
		},
		{
			hotkey: outlineHotkeys.checkCommit.hotkey,
			callback: toggleSelectedCommitChecked,
			options: {
				conflictBehavior: "allow",
				enabled: defaultOutlineHotkeysEnabled && isSelectedCommit,
				target: ref,
				meta: outlineHotkeys.checkCommit.meta,
			},
		},
		{
			hotkey: outlineHotkeys.checkBranchCommits.hotkey,
			callback: toggleSelectedBranchChecked,
			options: {
				conflictBehavior: "allow",
				enabled: defaultOutlineHotkeysEnabled && isSelectedBranch,
				target: ref,
				meta: outlineHotkeys.checkBranchCommits.meta,
			},
		},
		{
			hotkey: outlineHotkeys.deleteCommit.hotkey,
			callback: deleteSelectedCommit,
			options: {
				conflictBehavior: "allow",
				enabled:
					defaultOutlineHotkeysEnabled && isSelectedCommit && !commitDiscardMutation.isPending,
				target: ref,
				meta: outlineHotkeys.deleteCommit.meta,
			},
		},
		{
			hotkey: outlineHotkeys.moveCommitUp.hotkey,
			callback: () => moveSelectedCommit(-1),
			options: {
				conflictBehavior: "allow",
				enabled: defaultOutlineHotkeysEnabled && isSelectedCommit && !commitMoveMutation.isPending,
				target: ref,
				meta: outlineHotkeys.moveCommitUp.meta,
			},
		},
		{
			hotkey: outlineHotkeys.moveCommitDown.hotkey,
			callback: () => moveSelectedCommit(1),
			options: {
				conflictBehavior: "allow",
				enabled: defaultOutlineHotkeysEnabled && isSelectedCommit && !commitMoveMutation.isPending,
				target: ref,
				meta: outlineHotkeys.moveCommitDown.meta,
			},
		},
		{
			hotkey: outlineHotkeys.pushStack.hotkey,
			callback: pushSelectedBranch,
			options: {
				conflictBehavior: "allow",
				enabled: defaultOutlineHotkeysEnabled && canPushSelectedBranch,
				target: ref,
				meta: outlineHotkeys.pushStack.meta,
			},
		},
		{
			hotkey: outlineHotkeys.updateStack.hotkey,
			callback: updateSelectedStack,
			options: {
				conflictBehavior: "allow",
				enabled:
					defaultOutlineHotkeysEnabled &&
					!!selectedStackRebaseUpdate &&
					!workspaceIntegrateUpstreamMutation.isPending,
				target: ref,
				meta: outlineHotkeys.updateStack.meta,
			},
		},
		{
			hotkey: outlineHotkeys.insertEmptyCommit.hotkey,
			callback: insertEmptyCommit,
			options: {
				conflictBehavior: "allow",
				enabled:
					defaultOutlineHotkeysEnabled &&
					(isSelectedBranch || isSelectedCommit) &&
					!commitInsertBlankMutation.isPending,
				target: ref,
				meta: outlineHotkeys.insertEmptyCommit.meta,
			},
		},
		...Match.value(selection).pipe(
			Match.tags({
				Commit: (selection): RelativeTo => ({ type: "commit", subject: selection.commitId }),
				Branch: (selection): RelativeTo => ({
					type: "referenceBytes",
					subject: selection.branchRef,
				}),
			}),
			Match.orElse(() => null),
			(relativeTo) =>
				relativeTo
					? [
							{
								hotkey: outlineHotkeys.createDependentBranchAbove.hotkey,
								callback: () => createDependentBranchAbove(relativeTo),
								options: {
									conflictBehavior: "allow",
									enabled: defaultOutlineHotkeysEnabled,
									target: ref,
									meta: outlineHotkeys.createDependentBranchAbove.meta,
									requireReset: true,
								},
							} satisfies UseHotkeyDefinition,
							{
								hotkey: outlineHotkeys.composeCommitHere.hotkey,
								callback: () => composeCommitHere(relativeTo),
								options: {
									conflictBehavior: "allow",
									enabled: defaultOutlineHotkeysEnabled,
									target: ref,
									meta: outlineHotkeys.composeCommitHere.meta,
								},
							} satisfies UseHotkeyDefinition,
							{
								hotkey: outlineHotkeys.setCommitTarget.hotkey,
								callback: () => setCommitTarget(relativeTo),
								options: {
									conflictBehavior: "allow",
									enabled: defaultOutlineHotkeysEnabled,
									target: ref,
									meta: outlineHotkeys.setCommitTarget.meta,
								},
							} satisfies UseHotkeyDefinition,
						]
					: [],
		),
		{
			hotkey: outlineHotkeys.absorb.hotkey,
			callback: () => {
				enterAbsorbMode(changesSectionOperand, { type: "all" });
			},
			options: {
				conflictBehavior: "allow",
				enabled: defaultOutlineHotkeysEnabled && isSelectedChanges,
				target: ref,
				meta: outlineHotkeys.absorb.meta,
			},
		},
	]);
};

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

	const ref = useRef<HTMLDivElement>(null);

	useOutlineTreeHotkeys({
		navigationIndex,
		projectId,
		ref,
	});

	const commitTargetState = useAppSelector((state) => selectProjectCommitTarget(state, projectId));
	const targetComboboxItems = buildCommitTargetComboboxItems({ headInfo, commitTargetState });
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
						className={classes(
							props.className,
							styles.tree,
							hasCheckedCommits && styles.treeWithCheckedCommits,
						)}
						ref={useMergedRefs(refProp, ref)}
					>
						<div className={styles.changesContainer}>
							<Changes
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

const useIsSelected = ({
	projectId,
	operand,
}: {
	projectId: string;
	operand: Operand;
}): boolean => {
	const navigationIndex = assert(use(NavigationIndexContext));
	return useAppSelector((state) => {
		const selectionState = selectProjectSelectionOutline(state, projectId);
		const selection = resolveNavigationIndexSelection(
			navigationIndex,
			selectionState,
			operandIdentityKey,
		);

		return selection ? operandEquals(selection, operand) : false;
	});
};

const treeItemId = (operand: Operand): string =>
	`outline-treeitem-${encodeURIComponent(operandIdentityKey(operand))}`;

const CommitTargetIndicator: FC = () => (
	<Tooltip.Root>
		<Tooltip.Trigger aria-label="Commit target" className={styles.commitTargetIndicator}>
			<svg
				aria-hidden
				xmlns="http://www.w3.org/2000/svg"
				width="20"
				height="13"
				viewBox="0 0 20 13"
				fill="none"
			>
				<path
					d="M11.7571 11.7906C10.5268 12.5802 9.09568 13 7.63376 13L6.5 13C2.91015 13 -1.65294e-06 10.0898 -1.3391e-06 6.5C-1.02527e-06 2.91015 2.91015 4.13306e-07 6.5 7.27141e-07L7.63377 8.26258e-07C9.09568 9.54062e-07 10.5268 0.419776 11.7571 1.20943L18.6888 5.65843C19.3019 6.05196 19.3019 6.94804 18.6888 7.34157L11.7571 11.7906Z"
					fill="#25B1B1"
				/>
				<circle cx="6.5" cy="6.5" r="3.75" stroke="var(--bg-1)" strokeWidth="1.5" />
				<circle cx="6.5" cy="6.5" r="0.75" stroke="var(--bg-1)" strokeWidth="1.5" />
			</svg>
		</Tooltip.Trigger>
		<Tooltip.Portal>
			<Tooltip.Positioner sideOffset={4}>
				<Tooltip.Popup render={<TooltipPopup />}>Commit target</Tooltip.Popup>
			</Tooltip.Positioner>
		</Tooltip.Portal>
	</Tooltip.Root>
);

const ItemRow: FC<
	{
		projectId: string;
		operand: Operand;
		isCommitTarget?: boolean;
	} & Omit<ComponentProps<typeof WorkspaceItemRow>, "inert" | "isSelected" | "onSelect">
> = ({ projectId, operand, isCommitTarget, ...props }) => {
	const dispatch = useAppDispatch();
	const navigationIndex = assert(use(NavigationIndexContext));
	const isSelected = useIsSelected({ projectId, operand });
	const selectItem = () => {
		dispatch(projectActions.selectOutline({ projectId, selection: operand }));
	};

	return (
		<div className={styles.itemRowContainer}>
			<WorkspaceItemRow
				{...props}
				inert={!navigationIndexIncludes(navigationIndex, operand, operandIdentityKey)}
				isSelected={isSelected}
				onSelect={selectItem}
			/>
			{isCommitTarget && <CommitTargetIndicator />}
		</div>
	);
};

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
	const dispatch = useAppDispatch();
	const isSelected = useIsSelected({ projectId, operand });
	const absorptionTargetKeys = assert(use(AbsorptionTargetKeysContext));
	const isAbsorptionTarget = absorptionTargetKeys.has(operandIdentityKey(operand));
	const navigationIndex = assert(use(NavigationIndexContext));

	return useRender({
		render: (
			<OperationSourceC
				projectId={projectId}
				source={operand}
				onDragStart={() =>
					dispatch(projectActions.selectOutline({ projectId, selection: operand }))
				}
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

const InlineEditor: FC<{
	value: string;
	label: string;
	hotkeyGroup: CommandGroup;
	onMount?: (el: HTMLTextAreaElement | HTMLInputElement) => void;
	onSubmit: (value: string) => void;
	onExit: () => void;
	multiline: boolean;
	heading?: boolean;
}> = ({ value, label, hotkeyGroup, onMount, onSubmit, onExit, multiline, heading }) => {
	const name = useId();
	const formRef = useRef<HTMLFormElement | null>(null);
	const textFieldRef = useRef<HTMLTextAreaElement | HTMLInputElement>(null);
	const submitAction = (formData: FormData) => {
		onExit();
		onSubmit(formData.get(name) as string);
	};

	useHotkey("Enter", () => formRef.current?.requestSubmit(), {
		conflictBehavior: "allow",
		ignoreInputs: false,
		meta: { group: hotkeyGroup, name: "Save" },
		target: textFieldRef,
	});

	useHotkey("Escape", onExit, {
		conflictBehavior: "allow",
		ignoreInputs: false,
		meta: { group: hotkeyGroup, name: "Cancel" },
		target: textFieldRef,
	});

	const allTextFieldRefs = useMergedRefs(textFieldRef, (el) => {
		if (!el) return;
		el.focus();
		onMount?.(el);
	});

	return (
		<form ref={formRef} className={styles.inlineEditorForm} action={submitAction}>
			<WorkspaceItemRowLabelContainer>
				<WorkspaceItemRowLabel
					heading={heading}
					aria-label={label}
					className={styles.inlineEditorInput}
					render={
						multiline ? (
							<textarea ref={allTextFieldRefs} name={name} defaultValue={value} />
						) : (
							<input ref={allTextFieldRefs} name={name} defaultValue={value} />
						)
					}
				/>
			</WorkspaceItemRowLabelContainer>
			<div className={styles.inlineEditorHelp}>
				<button type="submit" className={getWorkspaceItemRowButtonClassName({})}>
					<kbd>{formatForDisplay("Enter")}</kbd>
					<span className={styles.inlineEditorShortcutLabel}> to Save</span>
				</button>
				<button type="button" className={getWorkspaceItemRowButtonClassName({})} onClick={onExit}>
					<kbd>{formatForDisplay("Escape")}</kbd>
					<span className={styles.inlineEditorShortcutLabel}> to Cancel</span>
				</button>
			</div>
		</form>
	);
};

const insertBlankCommitMenuItem = (
	insertBlankCommit: (side: "above" | "below") => void,
	acceleratorSide: "above" | "below",
) =>
	nativeMenuItem({
		label: "Add Empty Commit",
		submenu: [
			nativeMenuItem({
				label: "Above",
				accelerator:
					acceleratorSide === "above"
						? toElectronAccelerator(outlineHotkeys.insertEmptyCommit.hotkey)
						: undefined,
				onSelect: () => insertBlankCommit("above"),
			}),
			nativeMenuItem({
				label: "Below",
				accelerator:
					acceleratorSide === "below"
						? toElectronAccelerator(outlineHotkeys.insertEmptyCommit.hotkey)
						: undefined,
				onSelect: () => insertBlankCommit("below"),
			}),
		],
	});

const CommitRow: FC<
	{
		commit: Commit;
		projectId: string;
		stackId: string;
		isCommitTarget: boolean;
		dryRunCommit: Commit | null;
	} & ComponentProps<"div">
> = ({ commit, projectId, stackId, isCommitTarget, dryRunCommit, ...restProps }) => {
	const isHighlighted = useAppSelector((state) =>
		selectProjectHighlightedCommitIds(state, projectId).includes(commit.id),
	);
	const isChecked = useAppSelector((state) =>
		selectProjectCommitChecked(state, projectId, commit.id),
	);

	const dispatch = useAppDispatch();
	const navigationIndex = assert(use(NavigationIndexContext));
	const commitOperandV: CommitOperand = {
		stackId,
		commitId: commit.id,
	};
	const operand = commitOperand(commitOperandV);
	const isDefaultMode = useAppSelector(
		(state) => selectProjectOutlineModeState(state, projectId)._tag === "Default",
	);
	const isRewording = useAppSelector((state) => {
		const outlineMode = selectProjectOutlineModeState(state, projectId);
		return (
			outlineMode._tag === "RewordCommit" &&
			operandEquals(operand, commitOperand(outlineMode.operand))
		);
	});
	const [optimisticMessage, setOptimisticMessage] = useOptimistic(
		commit.message,
		(_currentMessage, nextMessage: string) => nextMessage,
	);
	const [isCommitMessagePending, startCommitMessageTransition] = useTransition();

	const commitWithOptimisticMessage: Commit = {
		...commit,
		message: optimisticMessage,
	};
	const { hasConflicts } = dryRunCommit ? dryRunCommit : commitWithOptimisticMessage;

	const commitInsertBlankMutation = useCommitInsertBlank();
	const commitDiscardMutation = useCommitDiscard();
	const commitUncommitMutation = useCommitUncommit();
	const commitRewordMutation = useCommitReword();
	const commitAmendMutation = useCommitAmend({ projectId });
	const branchCreateMutation = useBranchCreate();

	const insertBlankCommit = (side: "above" | "below") => {
		commitInsertBlankMutation.mutate({
			projectId,
			relativeTo: { type: "commit", subject: commit.id },
			side,
			dryRun: false,
		});
	};

	const createDependentBranch = (side: "above" | "below") => {
		branchCreateMutation.mutate(
			{
				projectId,
				newRef: null,
				placement: {
					type: "dependent",
					subject: {
						relativeTo: { type: "commit", subject: commit.id },
						side,
					},
				},
			},
			{
				onSuccess: (response) => {
					const newBranch = findBranchOperandByRef({
						headInfo: response.workspace.headInfo,
						branchRef: response.newRef.fullNameBytes,
					});
					if (newBranch)
						dispatch(
							projectActions.selectOutline({
								projectId,
								selection: branchOperand(newBranch),
							}),
						);
				},
			},
		);
	};

	const deleteCommit = () => {
		const selectionAfterDiscard = selectAfterDiscardedCommit({
			navigationIndex,
			commit: commitOperandV,
		});

		commitDiscardMutation.mutate(
			{
				projectId,
				subjectCommitId: commit.id,
				dryRun: false,
			},
			{
				onSuccess: (response) => {
					dispatch(
						projectActions.selectOutline({
							projectId,
							selection: rewrittenCommitSelection({
								selection: selectionAfterDiscard,
								replacedCommits: response.workspace.replacedCommits,
								headInfo: response.workspace.headInfo,
							}),
						}),
					);
				},
			},
		);
	};

	const cutCommit = () => {
		dispatch(
			projectActions.enterTransferMode({
				projectId,
				mode: keyboardTransferOperationMode({
					source: operand,
					operationType: "into",
				}),
			}),
		);
	};

	const startEditing = () => {
		dispatch(projectActions.startRewordCommit({ projectId, commit: commitOperandV }));
	};

	const endEditing = () => {
		dispatch(projectActions.exitMode({ projectId }));
		dispatch(projectActions.selectOutline({ projectId, selection: operand }));
		focusSelectionScope("outline");
	};

	const toastManager = Toast.useToastManager();

	const saveNewMessage = (newMessage: string) => {
		const initialMessage = commit.message.trim();
		const trimmed = newMessage.trim();
		if (trimmed === initialMessage) return;
		startCommitMessageTransition(async () => {
			setOptimisticMessage(trimmed);
			try {
				await commitRewordMutation.mutateAsync({
					projectId,
					commitId: commit.id,
					message: trimmed,
					dryRun: false,
				});
			} catch (error) {
				// oxlint-disable-next-line no-console
				console.error(error);

				toastManager.add({
					type: "error",
					title: "Failed to reword commit",
					description: errorMessageForToast(error),
					priority: "high",
				});
			}
		});
	};

	const relativeTo: RelativeTo = { type: "commit", subject: commit.id };

	const amendCommit = () => {
		commitAmendMutation.mutate({ commitId: commit.id });
	};

	const setCommitTarget = () => {
		dispatch(projectActions.setCommitTarget({ projectId, commitTarget: relativeTo }));
	};

	const composeCommitHere = () => {
		setCommitTarget();
		focusCommitMessageInput();
	};

	const title = commitTitle(commitWithOptimisticMessage.message);
	const body = commitBody(commitWithOptimisticMessage.message);

	const menuItems: Array<NativeMenuItem> = [
		nativeMenuItem({
			label: "Reword Commit",
			enabled: !isCommitMessagePending,
			accelerator: toElectronAccelerator(outlineHotkeys.rewordCommit.hotkey),
			onSelect: startEditing,
		}),
		nativeMenuItem({
			label: "Amend Commit",
			accelerator: toElectronAccelerator(outlineHotkeys.amendCommit.hotkey),
			enabled: isDefaultMode && !commitAmendMutation.isPending,
			onSelect: amendCommit,
		}),
		nativeMenuItem({
			label: "Cut Commit",
			onSelect: cutCommit,
			accelerator: toElectronAccelerator(selectionOperationHotkeys.cut.hotkey),
		}),
		nativeMenuSeparator,
		nativeMenuItem({
			label: "Compose Commit Here",
			accelerator: toElectronAccelerator(outlineHotkeys.composeCommitHere.hotkey),
			onSelect: composeCommitHere,
			enabled: isDefaultMode,
		}),
		nativeMenuItem({
			label: "Set Commit Target",
			accelerator: toElectronAccelerator(outlineHotkeys.setCommitTarget.hotkey),
			onSelect: setCommitTarget,
			enabled: isDefaultMode,
		}),
		nativeMenuItem({
			label: "Copy",
			submenu: [
				nativeMenuItem({
					label: "Change ID",
					onSelect: () => window.lite.clipboardWriteText(commit.changeId),
				}),
				nativeMenuItem({
					label: "Commit ID",
					onSelect: () => window.lite.clipboardWriteText(commit.id),
				}),
				nativeMenuItem({
					label: "Commit Title",
					enabled: title !== undefined,
					onSelect: () => window.lite.clipboardWriteText(title ?? ""),
				}),
				nativeMenuItem({
					label: "Commit Body",
					enabled: body !== undefined,
					onSelect: () => window.lite.clipboardWriteText(body ?? ""),
				}),
			],
		}),
		insertBlankCommitMenuItem(insertBlankCommit, "above"),
		nativeMenuSeparator,
		nativeMenuItem({
			label: "Create Branch",
			submenu: [
				nativeMenuItem({
					label: "Above",
					accelerator: toElectronAccelerator(outlineHotkeys.createDependentBranchAbove.hotkey),
					onSelect: () => createDependentBranch("above"),
				}),
				nativeMenuItem({
					label: "Below",
					onSelect: () => createDependentBranch("below"),
				}),
			],
		}),
		nativeMenuSeparator,
		nativeMenuItem({
			label: "Delete Commit",
			enabled: !commitDiscardMutation.isPending,
			accelerator: toElectronAccelerator(outlineHotkeys.deleteCommit.hotkey),
			onSelect: deleteCommit,
		}),
		nativeMenuItem({
			label: "Uncommit",
			enabled: !commitUncommitMutation.isPending,
			onSelect: () =>
				commitUncommitMutation.mutate({
					projectId,
					assignTo: null,
					subjectCommitIds: [commit.id],
					dryRun: false,
				}),
		}),
	];

	return (
		<ItemRow
			{...restProps}
			projectId={projectId}
			operand={operand}
			isHighlighted={isHighlighted}
			onContextMenu={(event) => {
				void showNativeContextMenu(event, menuItems);
			}}
			className={classes(restProps.className, styles.commitRow)}
			isCommitTarget={isCommitTarget}
		>
			<div className={styles.graphSegmentWithCheckbox}>
				<GraphSegment
					glyph="commit"
					status={commitIsDiverged(commit) ? "Diverged" : commit.state.type}
				/>
				<Tooltip.Root
					// This gets in the way when the user tries to move their hover to a
					// sibling row.
					disableHoverablePopup
				>
					<Checkbox
						disabled={!isDefaultMode}
						aria-label={`Check commit ${title ?? "(no message)"}`}
						checked={isChecked}
						className={styles.commitCheckbox}
						nativeButton
						render={<Tooltip.Trigger />}
						onCheckedChange={(checked) => {
							dispatch(
								projectActions.setCommitChecked({ projectId, commitId: commit.id, checked }),
							);
						}}
					/>
					<Tooltip.Portal>
						<Tooltip.Positioner sideOffset={4}>
							<Tooltip.Popup render={<TooltipPopup kbd={outlineHotkeys.checkCommit.hotkey} />}>
								{outlineHotkeys.checkCommit.meta.name}
							</Tooltip.Popup>
						</Tooltip.Positioner>
					</Tooltip.Portal>
				</Tooltip.Root>
			</div>

			{isRewording ? (
				<InlineEditor
					multiline
					value={optimisticMessage.trim()}
					label="Commit message"
					hotkeyGroup="Reword commit"
					onMount={(el) => {
						const firstNewline = el.value.indexOf("\n");
						const cursorPosition = firstNewline !== -1 ? firstNewline : el.value.length;
						el.setSelectionRange(cursorPosition, cursorPosition);
					}}
					onSubmit={saveNewMessage}
					onExit={endEditing}
				/>
			) : (
				<WorkspaceItemRowLabelContainer>
					<WorkspaceItemRowLabel singleLine>
						{title === undefined ? (
							<span className={workspaceItemRowStyles.fadedText}>(no message)</span>
						) : (
							title
						)}
						{hasConflicts && " ⚠️"}
					</WorkspaceItemRowLabel>
				</WorkspaceItemRowLabelContainer>
			)}

			{isDefaultMode && (
				<Toolbar.Root aria-label="Commit actions" render={<WorkspaceItemRowToolbar />}>
					<Toolbar.Button
						aria-label="Commit menu"
						onClick={(event) => {
							void showNativeMenuFromTrigger(event.currentTarget, menuItems);
						}}
						className={getWorkspaceItemRowButtonClassName({ iconOnly: true })}
					>
						<Icon name="kebab" />
					</Toolbar.Button>
				</Toolbar.Root>
			)}
		</ItemRow>
	);
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
						/>
					}
				/>
			}
		/>
	);
};

const ChangesSectionRow: FC<{
	changes: Array<TreeChange>;
	lineStats: LineStats | null;
	projectId: string;
}> = ({ changes, lineStats, projectId }) => {
	const operand = changesSectionOperand;
	const isDefaultMode = useAppSelector(
		(state) => selectProjectOutlineModeState(state, projectId)._tag === "Default",
	);
	const discardWorktreeChanges = useDiscardWorktreeChanges();

	const dispatch = useAppDispatch();
	const enterAbsorbMode = (source: Operand, sourceTarget: AbsorptionTarget) => {
		dispatch(projectActions.enterAbsorbMode({ projectId, source, sourceTarget }));
	};

	const absorb = () => {
		enterAbsorbMode(operand, { type: "all" });
	};

	const composeCommitMessage = () => {
		dispatch(projectActions.selectOutline({ projectId, selection: changesSectionOperand }));
		focusCommitMessageInput();
	};

	const discardChanges = () => {
		discardWorktreeChanges.mutate({
			projectId,
			changes: changes.map((change) => createDiffSpec(change, [])),
		});
	};

	const menuItems: Array<NativeMenuItem> = [
		nativeMenuItem({
			label: "Compose Commit Message",
			accelerator: toElectronAccelerator(outlineHotkeys.composeCommitMessageFromChanges.hotkey),
			onSelect: composeCommitMessage,
			enabled: isDefaultMode,
		}),
		nativeMenuSeparator,
		nativeMenuItem({
			label: "Absorb",
			accelerator: toElectronAccelerator(outlineHotkeys.absorb.hotkey),
			onSelect: absorb,
		}),
		nativeMenuItem({
			label: "Discard Changes",
			enabled: changes.length > 0 && !discardWorktreeChanges.isPending,
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
			<WorkspaceItemRowLabelContainer>
				<WorkspaceItemRowLabel heading>
					{changes.length === 0 ? "Nothing to commit" : "Uncommitted changes"}
				</WorkspaceItemRowLabel>

				<span
					className={classes(
						"text-11",
						"text-semibold",
						workspaceItemRowStyles.bubble,
						workspaceItemRowStyles.changesCountBubble,
					)}
				>
					{changes.length}
				</span>

				{lineStats && (lineStats.linesAdded > 0 || lineStats.linesRemoved > 0) && (
					<span className={workspaceItemRowStyles.lineStatsGroup}>
						{lineStats.linesAdded > 0 && (
							<span
								className={classes(
									"text-11",
									"text-semibold",
									workspaceItemRowStyles.bubble,
									workspaceItemRowStyles.lineStatsBubble,
									workspaceItemRowStyles.lineStatsAdded,
								)}
							>
								+{lineStats.linesAdded}
							</span>
						)}
						{lineStats.linesRemoved > 0 && (
							<span
								className={classes(
									"text-11",
									"text-semibold",
									workspaceItemRowStyles.bubble,
									workspaceItemRowStyles.lineStatsBubble,
									workspaceItemRowStyles.lineStatsRemoved,
								)}
							>
								-{lineStats.linesRemoved}
							</span>
						)}
					</span>
				)}
			</WorkspaceItemRowLabelContainer>

			{isDefaultMode && (
				<Toolbar.Root
					aria-label="Changes actions"
					render={<WorkspaceItemRowToolbar forceVisible />}
				>
					<Toolbar.Button
						aria-label="Changes menu"
						onClick={(event) => {
							void showNativeMenuFromTrigger(event.currentTarget, menuItems);
						}}
						className={getWorkspaceItemRowButtonClassName({ iconOnly: true })}
					>
						<Icon name="kebab" />
					</Toolbar.Button>
				</Toolbar.Root>
			)}
		</ItemRow>
	);
};

const relativeToKey = (relativeTo: RelativeTo): string => {
	switch (relativeTo.type) {
		case "reference":
			return JSON.stringify(["reference", relativeTo.subject]);
		case "referenceBytes":
			return JSON.stringify(["referenceBytes", relativeTo.subject]);
		case "commit":
			return JSON.stringify(["commit", relativeTo.subject]);
	}
};

const relativeToEquals = (a: RelativeTo, b: RelativeTo): boolean =>
	relativeToKey(a) === relativeToKey(b);

type CommitTargetComboboxItem = {
	label: string;
	relativeTo: RelativeTo;
};

const buildCommitTargetComboboxItems = ({
	headInfo,
	commitTargetState,
}: {
	headInfo: RefInfo | undefined;
	commitTargetState: RelativeTo | null;
}): Array<CommitTargetComboboxItem> => {
	const commitTarget =
		headInfo && commitTargetState?.type === "commit"
			? findCommit({ headInfo, commitId: commitTargetState.subject })
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

const Changes: FC<{
	projectId: string;
	commitTarget: CommitTargetComboboxItem | null;
	targetComboboxItems: Array<CommitTargetComboboxItem>;
}> = ({ projectId, commitTarget, targetComboboxItems }) => {
	const dispatch = useAppDispatch();
	const commitCreateMutation = useCommitCreate({ projectId });
	const commitAmendMutation = useCommitAmend({ projectId });

	const { data: worktreeChanges } = useQuery(changesInWorktreeQueryOptions(projectId));
	const treeChangeDiffs = useQueries({
		queries:
			worktreeChanges?.changes.map((change) =>
				treeChangeDiffsQueryOptions({ projectId, change }),
			) ?? [],
	});
	const lineStats = getLineStats(treeChangeDiffs.map((result) => result.data));

	const operand = changesSectionOperand;
	const commitTextareaRef = useRef<HTMLTextAreaElement | null>(null);

	const isDefaultMode = useAppSelector(
		(state) => selectProjectOutlineModeState(state, projectId)._tag === "Default",
	);

	const { data: headInfo } = useQuery(headInfoQueryOptions(projectId));
	const isCommitOrAmendPending = commitCreateMutation.isPending || commitAmendMutation.isPending;
	const canCommitOrAmendBase = isDefaultMode && commitTarget !== null && !isCommitOrAmendPending;
	const canCommit = canCommitOrAmendBase;
	const canAmend =
		canCommitOrAmendBase &&
		worktreeChanges &&
		worktreeChanges.changes.length > 0 &&
		headInfo &&
		resolveRelativeTo({ headInfo, relativeTo: commitTarget.relativeTo }) !== null;

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
		if (!commitTarget || !headInfo) return;

		const commitId = resolveRelativeTo({
			headInfo,
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
				meta: changesHotkeys.selectCommitTarget.meta,
			},
		},
		{
			hotkey: changesHotkeys.commit.hotkey,
			callback: createCommit,
			options: {
				conflictBehavior: "allow",
				enabled: canCommit,
				ignoreInputs: false,
				meta: changesHotkeys.commit.meta,
			},
		},
		{
			hotkey: changesHotkeys.amendCommit.hotkey,
			callback: amendCommit,
			options: {
				conflictBehavior: "allow",
				enabled: canAmend,
				ignoreInputs: false,
				meta: changesHotkeys.amendCommit.meta,
			},
		},
	]);

	const focusCommitMessageHotkeyLabel = formatForDisplay(
		outlineHotkeys.composeCommitMessage.hotkey,
	);

	return (
		<TreeItem
			projectId={projectId}
			operand={operand}
			aria-label={`Changes (${worktreeChanges?.changes.length ?? 0})`}
			className={classes(styles.section, styles.changesSection)}
			render={
				<OperandC projectId={projectId} operand={operand} render={<form onSubmit={submit} />} />
			}
		>
			<ChangesSectionRow
				changes={worktreeChanges?.changes ?? []}
				lineStats={lineStats}
				projectId={projectId}
			/>

			<div className={styles.commitControls}>
				<textarea
					id={commitMessageInputId}
					ref={commitTextareaRef}
					aria-label="Compose commit message"
					disabled={!isDefaultMode}
					readOnly={isCommitOrAmendPending}
					placeholder={`Compose commit message ${focusCommitMessageHotkeyLabel}`}
					className={classes("text-13", styles.commitTextarea)}
					onFocus={selectChanges}
					onKeyDown={(event) => {
						if (event.key !== "Escape") return;
						event.preventDefault();
						focusSelectionScope("outline");
					}}
				/>

				<div className={styles.commitControlsFooter}>
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
								className={classes("text-13", styles.commitTargetComboboxTrigger)}
								aria-label={changesHotkeys.selectCommitTarget.meta.name}
								// We pass `disabled` here because we want to disable the button, not
								// the tooltip. Other props should be passed above.
								render={<Button focusableWhenDisabled render={<Tooltip.Trigger />} />}
							>
								<Icon name="bullseye" size={14} />
								<span className={styles.commitTargetComboboxTriggerLabel}>
									<Combobox.Value placeholder={changesHotkeys.selectCommitTarget.meta.name} />
								</span>
							</Combobox.Trigger>
							<Tooltip.Portal>
								<Tooltip.Positioner sideOffset={4}>
									<Tooltip.Popup
										render={<TooltipPopup kbd={changesHotkeys.selectCommitTarget.hotkey} />}
									>
										{changesHotkeys.selectCommitTarget.meta.name}
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
			</div>
		</TreeItem>
	);
};

const pushStatusRequiresPush = (pushStatus: PushStatus): boolean =>
	pushStatus === "unpushedCommits" ||
	pushStatus === "unpushedCommitsRequiringForce" ||
	pushStatus === "completelyUnpushed";

type PartialStackState = {
	requiresPush: boolean;
	pushWithForce: boolean;
	hasConflicts: boolean;
	commitsCount: number;
	branchCount: number;
};

const emptyPartialStackState: PartialStackState = {
	requiresPush: false,
	pushWithForce: false,
	hasConflicts: false,
	commitsCount: 0,
	branchCount: 0,
};

const addSegmentToPartialStackState = (
	state: PartialStackState,
	segment: Segment,
): PartialStackState => ({
	requiresPush: state.requiresPush || pushStatusRequiresPush(segment.pushStatus),
	pushWithForce: state.pushWithForce || segment.pushStatus === "unpushedCommitsRequiringForce",
	hasConflicts: state.hasConflicts || segment.commits.some((commit) => commit.hasConflicts),
	commitsCount: state.commitsCount + segment.commits.length,
	branchCount: segment.refName ? state.branchCount + 1 : state.branchCount,
});

const partialStackPushDisabledReason = (partialStackState: PartialStackState): string | null =>
	partialStackState.hasConflicts
		? "Resolve conflicts before pushing"
		: !partialStackState.requiresPush || partialStackState.commitsCount === 0
			? "Nothing to push"
			: null;

const partialStackStateFromSegments = (segments: Array<Segment>): PartialStackState =>
	segments.reduce(addSegmentToPartialStackState, emptyPartialStackState);

const partialStackStatesFromSegments = (segments: Array<Segment>): Array<PartialStackState> =>
	initNonEmpty(scanRight(segments, emptyPartialStackState, addSegmentToPartialStackState));

type PushContext = {
	refName: BranchReference;
	partialStackSegments: Array<Segment>;
};

const pushContextForSegment = ({
	segments,
	segmentIndex,
}: {
	segments: Array<Segment>;
	segmentIndex: number;
}): PushContext | null => {
	const segment = segments[segmentIndex];
	if (!segment?.refName) return null;

	const partialStackSegments = segments.slice(segmentIndex);

	return {
		refName: segment.refName,
		partialStackSegments,
	};
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

const BranchRow: FC<
	{
		projectId: string;
		refName: BranchReference;
		stackId: string;
		isCommitTarget: boolean;
		canTearOffBranch: boolean;
		canRemoveBranch: boolean;
		partialStackState: PartialStackState;
		pushStatus: PushStatus;
		pullRequest: number | null;
		bottomRelativeTo: RelativeTo | null;
		isTopSegment: boolean;
	} & ComponentProps<"div">
> = ({
	projectId,
	refName,
	stackId,
	isCommitTarget,
	canTearOffBranch,
	canRemoveBranch,
	partialStackState,
	pushStatus,
	pullRequest,
	bottomRelativeTo,
	isTopSegment,
	...restProps
}) => {
	const dispatch = useAppDispatch();
	const branchOperandV: BranchOperand = {
		stackId,
		branchRef: refName.fullNameBytes,
	};
	const operand = branchOperand(branchOperandV);
	const isDefaultMode = useAppSelector(
		(state) => selectProjectOutlineModeState(state, projectId)._tag === "Default",
	);
	const isRenaming = useAppSelector((state) => {
		const outlineMode = selectProjectOutlineModeState(state, projectId);
		return (
			outlineMode._tag === "RenameBranch" &&
			operandEquals(operand, branchOperand(outlineMode.operand))
		);
	});
	const [optimisticBranchDisplayName, setOptimisticBranchDisplayName] = useOptimistic(
		refName.displayName,
		(_currentBranchName, nextBranchName: string) => nextBranchName,
	);
	const [isRenamePending, startRenameTransition] = useTransition();

	const updateBranchNameMutation = useUpdateBranchName({
		projectId,
		stackId,
		branchRef: refName.fullNameBytes,
		oldBranch: branchOperandV,
	});

	const startEditing = () => {
		dispatch(projectActions.startRenameBranch({ projectId, branch: branchOperandV }));
	};

	const endEditing = () => {
		dispatch(projectActions.exitMode({ projectId }));
		dispatch(projectActions.selectOutline({ projectId, selection: operand }));
		focusSelectionScope("outline");
	};

	const toastManager = Toast.useToastManager();

	const pushStackMutation = usePushStack();
	const commitInsertBlankMutation = useCommitInsertBlank();
	const tearOffBranchMutation = useTearOffBranch();
	const removeBranchMutation = useRemoveBranch();
	const branchCreateMutation = useBranchCreate();

	const pushesMultipleBranches = partialStackState.branchCount > 1;

	const saveBranchName = (newBranchName: string) => {
		const trimmed = newBranchName.trim();
		if (trimmed === "" || trimmed === refName.displayName) return;
		startRenameTransition(async () => {
			setOptimisticBranchDisplayName(trimmed);
			try {
				await updateBranchNameMutation.mutateAsync({
					projectId,
					stackId,
					branchName: refName.displayName,
					newName: trimmed,
				});
			} catch (error) {
				// oxlint-disable-next-line no-console
				console.error(error);

				toastManager.add({
					type: "error",
					title: "Failed to rename branch",
					description: errorMessageForToast(error),
					priority: "high",
				});
			}
		});
	};

	const relativeTo: RelativeTo = { type: "referenceBytes", subject: refName.fullNameBytes };
	const bucketRelativeTo = (side: InsertSide): RelativeTo =>
		side === "below" && bottomRelativeTo !== null ? bottomRelativeTo : relativeTo;

	const setCommitTarget = () => {
		dispatch(projectActions.setCommitTarget({ projectId, commitTarget: relativeTo }));
	};

	const composeCommitHere = () => {
		setCommitTarget();
		focusCommitMessageInput();
	};

	const insertBlankCommit = (side: "above" | "below") => {
		commitInsertBlankMutation.mutate({
			projectId,
			relativeTo,
			side,
			dryRun: false,
		});
	};

	const createDependentBranch = (side: "above" | "below") => {
		branchCreateMutation.mutate(
			{
				projectId,
				newRef: null,
				placement: {
					type: "dependent",
					subject: {
						relativeTo: bucketRelativeTo(side),
						side,
					},
				},
			},
			{
				onSuccess: (response) => {
					const newBranch = findBranchOperandByRef({
						headInfo: response.workspace.headInfo,
						branchRef: response.newRef.fullNameBytes,
					});
					if (newBranch)
						dispatch(
							projectActions.selectOutline({
								projectId,
								selection: branchOperand(newBranch),
							}),
						);
				},
			},
		);
	};

	const tearOffBranch = () => {
		tearOffBranchMutation.mutate({
			projectId,
			subjectBranch: decodeBytes(refName.fullNameBytes),
			dryRun: false,
		});
	};

	const pushStack = () => {
		pushStackMutation.mutate({
			projectId,
			branch: decodeBytes(refName.fullNameBytes),
			withForce: partialStackState.pushWithForce,
			skipForcePushProtection: false,
			runHooks: true,
			pushOpts: [],
		});
	};

	const pushStackDisabledReason = pushStackMutation.isPending
		? "Pushing"
		: partialStackPushDisabledReason(partialStackState);
	const canPushStack = pushStackDisabledReason === null;
	const pushButtonLabel = pushesMultipleBranches
		? partialStackState.pushWithForce
			? "Force push this and all branches below"
			: "Push this and all branches below"
		: partialStackState.pushWithForce
			? "Force push branch"
			: "Push branch";
	const pushMenuLabel = pushesMultipleBranches
		? partialStackState.pushWithForce
			? "Force Push With Branches Below"
			: "Push With Branches Below"
		: partialStackState.pushWithForce
			? "Force Push Branch"
			: "Push Branch";

	const menuItems: Array<NativeMenuItem> = [
		nativeMenuItem({
			label: pushMenuLabel,
			enabled: canPushStack,
			accelerator: toElectronAccelerator(outlineHotkeys.pushStack.hotkey),
			onSelect: pushStack,
		}),
		nativeMenuSeparator,
		nativeMenuItem({
			label: "Rename Branch",
			enabled: !isRenamePending,
			accelerator: toElectronAccelerator(outlineHotkeys.renameBranch.hotkey),
			onSelect: startEditing,
		}),
		nativeMenuItem({
			label: "Copy Branch Name",
			onSelect: () => window.lite.clipboardWriteText(optimisticBranchDisplayName),
		}),
		nativeMenuSeparator,
		nativeMenuItem({
			label: "Compose Commit Here",
			accelerator: toElectronAccelerator(outlineHotkeys.composeCommitHere.hotkey),
			onSelect: composeCommitHere,
			enabled: isDefaultMode,
		}),
		nativeMenuItem({
			label: "Set Commit Target",
			accelerator: toElectronAccelerator(outlineHotkeys.setCommitTarget.hotkey),
			onSelect: setCommitTarget,
			enabled: isDefaultMode,
		}),
		insertBlankCommitMenuItem(insertBlankCommit, "below"),
		nativeMenuSeparator,
		nativeMenuItem({
			label: "Create Branch",
			submenu: [
				nativeMenuItem({
					label: "Above",
					accelerator: toElectronAccelerator(outlineHotkeys.createDependentBranchAbove.hotkey),
					onSelect: () => createDependentBranch("above"),
				}),
				nativeMenuItem({
					label: "Below",
					onSelect: () => createDependentBranch("below"),
				}),
			],
		}),
		nativeMenuSeparator,
		nativeMenuItem({
			label: "Tear Off Branch",
			enabled: canTearOffBranch && !tearOffBranchMutation.isPending,
			onSelect: tearOffBranch,
		}),
		nativeMenuItem({
			label: "Delete Branch Reference",
			enabled: canRemoveBranch,
			onSelect: () =>
				removeBranchMutation.mutate({
					projectId,
					stackId,
					branchName: decodeBytes(refName.fullNameBytes),
				}),
		}),
	];

	return (
		<ItemRow
			{...restProps}
			projectId={projectId}
			operand={operand}
			onContextMenu={(event) => {
				void showNativeContextMenu(event, menuItems);
			}}
			isCommitTarget={isCommitTarget}
		>
			<GraphSegment
				glyph={isTopSegment ? "forkRight" : "joinRight"}
				status={segmentPushStatusToStatus(pushStatus)}
			/>

			{isRenaming ? (
				<InlineEditor
					multiline={false}
					heading
					value={optimisticBranchDisplayName}
					label="Branch name"
					hotkeyGroup="Rename branch"
					onMount={(el) => {
						el.select();
					}}
					onSubmit={saveBranchName}
					onExit={endEditing}
				/>
			) : (
				<div className={styles.branchLabel}>
					<WorkspaceItemRowLabelContainer>
						<WorkspaceItemRowLabel heading>{optimisticBranchDisplayName}</WorkspaceItemRowLabel>
					</WorkspaceItemRowLabelContainer>

					<div className={classes("text-13", styles.branchLabelMeta)}>
						<span className={workspaceItemRowStyles.fadedText}>
							{Match.value(pushStatus).pipe(
								Match.when("nothingToPush", () => "Nothing to push"),
								Match.when("unpushedCommits", () => "Some unpushed"),
								Match.when("completelyUnpushed", () => "Unpushed branch"),
								Match.when("unpushedCommitsRequiringForce", () => "Some unpushed"),
								Match.when("integrated", () => "Integrated"),
								Match.exhaustive,
							)}
						</span>

						{pullRequest !== null && (
							<span
								className={classes(workspaceItemRowStyles.fadedText, styles.branchLabelMetaItem)}
							>
								<Icon name="pr" />
								PR
							</span>
						)}

						<Tooltip.Root>
							<Tooltip.Trigger
								aria-label={pushButtonLabel}
								onClick={pushStack}
								className={getWorkspaceItemRowButtonClassName({ variant: "outline" })}
								// We pass `disabled` here because we want to disable the button, not
								// the tooltip. Other props should be passed above.
								render={<Button focusableWhenDisabled disabled={!canPushStack} />}
							>
								{pushStackMutation.isPending ? (
									<Icon name="spinner" />
								) : pushesMultipleBranches ? (
									<Icon name="arrow-double-line-up" />
								) : (
									<Icon name="arrow-line-up" />
								)}
								Push
							</Tooltip.Trigger>
							<Tooltip.Portal>
								<Tooltip.Positioner sideOffset={4}>
									<Tooltip.Popup render={<TooltipPopup kbd={outlineHotkeys.pushStack.hotkey} />}>
										{pushStackDisabledReason ?? pushButtonLabel}
									</Tooltip.Popup>
								</Tooltip.Positioner>
							</Tooltip.Portal>
						</Tooltip.Root>
					</div>
				</div>
			)}

			{isDefaultMode && (
				<Toolbar.Root aria-label="Branch actions" render={<WorkspaceItemRowToolbar forceVisible />}>
					<Toolbar.Button
						aria-label="Branch menu"
						onClick={(event) => {
							void showNativeMenuFromTrigger(event.currentTarget, menuItems);
						}}
						className={getWorkspaceItemRowButtonClassName({ iconOnly: true })}
					>
						<Icon name="kebab" />
					</Toolbar.Button>
				</Toolbar.Root>
			)}
		</ItemRow>
	);
};

const StackRow: FC<
	{
		projectId: string;
		stack: Stack;
	} & Omit<ComponentProps<"div">, "onSelect">
> = ({ projectId, stack, ...restProps }) => {
	const relativeTo = stackBottomRelativeTo(stack);
	const rebaseUpdate: BottomUpdate | null = relativeTo
		? { kind: "rebase", selector: relativeTo }
		: null;
	const isDefaultMode = useAppSelector(
		(state) => selectProjectOutlineModeState(state, projectId)._tag === "Default",
	);

	const unapplyStackMutation = useUnapplyStack();
	const unapply = () => {
		// oxlint-disable-next-line typescript/no-non-null-assertion -- [ref:stack-id-required]
		unapplyStackMutation.mutate({ projectId, stackId: stack.id! });
	};

	const workspaceIntegrateUpstreamMutation = useWorkspaceIntegrateUpstream();
	const updateStack = () => {
		if (rebaseUpdate)
			workspaceIntegrateUpstreamMutation.mutate({
				projectId,
				updates: [rebaseUpdate],
				dryRun: false,
			});
	};

	const menuItems: Array<NativeMenuItem> = [
		nativeMenuItem({ label: "Move Up", enabled: false }),
		nativeMenuItem({ label: "Move Down", enabled: false }),
		nativeMenuSeparator,
		nativeMenuItem({
			label: "Update Stack (Rebases)",
			enabled: !!rebaseUpdate,
			accelerator: toElectronAccelerator(outlineHotkeys.updateStack.hotkey),
			onSelect: updateStack,
		}),
		nativeMenuItem({
			label: "Unapply Stack",
			enabled: !unapplyStackMutation.isPending,
			onSelect: unapply,
		}),
	];

	return (
		<WorkspaceItemRow
			{...restProps}
			interactive={false}
			onContextMenu={(event) => {
				void showNativeContextMenu(event, menuItems);
			}}
		>
			<WorkspaceItemRowLabelContainer />

			{isDefaultMode && (
				<Toolbar.Root aria-label="Stack actions" render={<WorkspaceItemRowToolbar forceVisible />}>
					<Toolbar.Button
						aria-label="Stack menu"
						onClick={(event) => {
							void showNativeMenuFromTrigger(event.currentTarget, menuItems);
						}}
						className={getWorkspaceItemRowButtonClassName({ iconOnly: true })}
					>
						<Icon name="kebab" />
					</Toolbar.Button>
				</Toolbar.Root>
			)}
		</WorkspaceItemRow>
	);
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
				pullRequest={segment.metadata?.review.pullRequest ?? null}
				bottomRelativeTo={segmentBottomRelativeTo(segment)}
				isTopSegment={isTopSegment}
			/>

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

	return (
		<div>
			{segment.commits.map((commit) => {
				const dryRunCommitId = dryRunWorkspace?.replacedCommits[commit.id];
				const dryRunCommit =
					dryRunWorkspace && dryRunCommitId !== undefined
						? findCommit({ headInfo: dryRunWorkspace.headInfo, commitId: dryRunCommitId })
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
