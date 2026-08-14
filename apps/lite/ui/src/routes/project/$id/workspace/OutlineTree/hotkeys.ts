import {
	useBranchCreate,
	useBranchRemove,
	useCommitDiscard,
	useCommitInsertBlank,
	useCommitMove,
	useCommitUncommit,
	useWorkspaceBranchAndAncestorsPush,
	useWorkspaceIntegrateUpstream,
} from "#ui/api/mutations.ts";
import {
	setCursor,
	startRenameBranch,
	startRewordCommit,
	useResolvedCursor,
} from "#ui/use-cursor.ts";
import { forgeInfoOptions, headInfoQueryOptions } from "#ui/api/queries.ts";
import { decodeBytes } from "#ui/api/bytes.ts";
import { getHeadInfoIndex } from "#ui/api/ref-info.ts";
import { commitForgeUrl } from "#ui/commit.ts";
import { outlineHotkeys } from "#ui/hotkeys.ts";
import { branchOperand, commitOperand, operandIdentityKey, type Operand } from "#ui/operands.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { interfaceSlice } from "#ui/interface/state.ts";
import { useNavigationIndexHotkeys } from "#ui/selection-scopes.ts";
import { useAppDispatch, useAppSelector, useAppStore } from "#ui/store.ts";
import type { NavigationIndex } from "#ui/workspace/navigation-index.ts";
import { prForgeUrl } from "#ui/pr.ts";
import { stackBottomRelativeTo } from "#ui/api/stack.ts";
import type {
	BranchReference,
	BottomUpdate,
	InsertSide,
	RelativeTo,
	Segment,
} from "@gitbutler/but-sdk";
import { type UseHotkeyDefinition, useHotkeys } from "@tanstack/react-hotkeys";
import { useQuery } from "@tanstack/react-query";
import { Match } from "effect";
import type { RefObject } from "react";
import { toggleFoldedSegment } from "./fold.ts";
import { selectAfterDiscardedCommit } from "./selectAfterDiscardedCommit.ts";
import {
	canRemoveBranchReference,
	downstackPushStatusDisabled,
	downstackPushStatusFromSegments,
} from "#ui/segment.ts";

type PushContext = {
	refName: BranchReference;
	downstackSegments: Array<Segment>;
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

	const downstackSegments = segments.slice(segmentIndex);

	return {
		refName: segment.refName,
		downstackSegments,
	};
};

export const useOutlineTreeHotkeys = ({
	navigationIndex,
	projectId,
	ref,
	checkCommit,
	focusCommitMessageInput,
	onEdgeSpill,
}: {
	navigationIndex: NavigationIndex<Operand>;
	projectId: string;
	ref: RefObject<HTMLElement | null>;
	checkCommit: (evt: { commitId: string; shiftKey: boolean }) => void;
	focusCommitMessageInput: () => void;
	onEdgeSpill?: (offset: -1 | 1) => void;
}) => {
	const { data: headInfoIndex } = useQuery({
		...headInfoQueryOptions(projectId),
		select: getHeadInfoIndex,
	});
	const { data: forgeInfo } = useQuery(forgeInfoOptions(projectId));
	const store = useAppStore();
	const selection = useResolvedCursor("stacks", navigationIndex);
	const isDefaultMode = useAppSelector(
		(state) => projectSlice.selectors.selectOutlineModeState(state, projectId)._tag === "Default",
	);

	const selectionContext = Match.value(selection).pipe(
		Match.tags({
			Branch: (branch) => headInfoIndex?.branchContextByRefBytes(branch.branchRef),
			Commit: (commit) => headInfoIndex?.commitContextByCommitId(commit.commitId),
		}),
		Match.orElse(() => undefined),
	);
	const selectionStack = selectionContext?.stack;
	const selectedBranchSegment =
		selection?._tag === "Branch" ? selectionContext?.segment : undefined;
	// Only a segment with a branch reference and commits to hide can be folded.
	const foldableSegmentRef =
		selectionContext !== undefined &&
		selectionContext.segment.refName !== null &&
		selectionContext.segment.commits.length > 0
			? selectionContext.segment.refName
			: null;

	const selectedCommit =
		selection?._tag === "Commit"
			? (headInfoIndex?.commitContextByCommitId(selection.commitId) ?? null)?.commit
			: null;
	const selectedCommitForgeUrl =
		selectedCommit && forgeInfo ? commitForgeUrl(selectedCommit, forgeInfo) : null;
	const selectedBranchPullRequest = selectedBranchSegment?.metadata?.review.pullRequest ?? null;
	const selectedBranchPullRequestUrl =
		selectedBranchPullRequest !== null && forgeInfo
			? prForgeUrl(selectedBranchPullRequest, forgeInfo)
			: null;

	const dispatch = useAppDispatch();

	const { isPending: isCommitMovePending, mutate: commitMove } = useCommitMove();
	const { isPending: isCommitDiscardPending, mutate: commitDiscard } = useCommitDiscard();
	const { isPending: isCommitUncommitPending, mutate: commitUncommit } = useCommitUncommit();
	const { isPending: isCommitInsertBlankPending, mutate: commitInsertBlank } =
		useCommitInsertBlank();
	const {
		isPending: isWorkspaceBranchAndAncestorsPushPending,
		mutate: workspaceBranchAndAncestorsPush,
	} = useWorkspaceBranchAndAncestorsPush();
	const { isPending: isWorkspaceIntegrateUpstreamPending, mutate: workspaceIntegrateUpstream } =
		useWorkspaceIntegrateUpstream();
	const { mutate: branchCreate } = useBranchCreate();
	const { isPending: isBranchRemovePending, mutate: branchRemove } = useBranchRemove();

	const openBranchPicker = () => {
		dispatch(interfaceSlice.actions.openDialog({ dialog: { _tag: "BranchPicker" } }));
	};

	const insertEmptyCommit = (sideIntent: InsertSide) => {
		if (!selection) return;

		type Placement = { relativeTo: RelativeTo; side: InsertSide };
		const placement = Match.value(selection).pipe(
			Match.tags({
				Commit: (selection): Placement => ({
					relativeTo: { type: "commit", subject: selection.commitId },
					side: sideIntent,
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

		commitInsertBlank({
			projectId,
			relativeTo: placement.relativeTo,
			side: placement.side,
			dryRun: false,
		});
	};

	const createDependentBranchAbove = (relativeTo: RelativeTo) => {
		branchCreate(
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
					setCursor("stacks", branchOperand({ branchRef: response.newRef.fullNameBytes }));
				},
			},
		);
	};

	const toggleSelectedCommitChecked = (event: KeyboardEvent) => {
		if (!selection || selection._tag !== "Commit") return;
		// Leave activation of a directly focused checkbox to the checkbox itself.
		if (event.target !== ref.current) return;

		event.preventDefault();
		event.stopPropagation();
		checkCommit({
			commitId: selection.commitId,
			shiftKey: event.shiftKey,
		});
	};

	const toggleSelectedBranchChecked = () => {
		if (!selectedBranchSegment) return;

		const selectedBranchCommitsChecked =
			selectedBranchSegment.commits.length > 0
				? selectedBranchSegment.commits.every((commit) =>
						projectSlice.selectors
							.selectCheckedCommitIds(store.getState(), projectId)
							.has(commit.id),
					)
				: false;

		dispatch(
			projectSlice.actions.checkOperands({
				projectId,
				operands: selectedBranchSegment.commits.map((commit) =>
					commitOperand({ commitId: commit.id, changeId: commit.changeId }),
				),
				checked: !selectedBranchCommitsChecked,
			}),
		);
	};

	const moveSelectedCommit = (offset: -1 | 1) => {
		if (!selection || selection._tag !== "Commit") return;

		const source = commitOperand(selection);
		const selectionIdx = navigationIndex.indexByKey.get(operandIdentityKey(source));
		if (selectionIdx === undefined) return;

		const checkedCommitIds = projectSlice.selectors.selectCheckedCommitIds(
			store.getState(),
			projectId,
		);
		const subjectCommitIds =
			checkedCommitIds.size > 0 ? checkedCommitIds : new Set([selection.commitId]);

		let nextItemIndex = selectionIdx;
		let nextItem: Operand | undefined;
		do {
			nextItemIndex += offset;
			nextItem = navigationIndex.items[nextItemIndex];
		} while (nextItem?._tag === "Commit" && subjectCommitIds.has(nextItem.commitId));
		if (!nextItem) return;

		let relativeTo: RelativeTo;
		switch (nextItem._tag) {
			case "Commit":
				relativeTo = { type: "commit", subject: nextItem.commitId };
				break;
			case "Branch":
				relativeTo = { type: "referenceBytes", subject: nextItem.branchRef };
				break;
			default:
				throw new Error("Only commits and branches are valid outline items");
		}

		commitMove({
			projectId,
			subjectCommitIds: Array.from(subjectCommitIds),
			relativeTo,
			side: offset === -1 ? "above" : "below",
			dryRun: false,
		});
	};

	const deleteSelectedCommit = () => {
		if (!selection || selection._tag !== "Commit") return;

		const selectionAfterDiscard = selectAfterDiscardedCommit({
			navigationIndex,
			commit: { commitId: selection.commitId, changeId: selection.changeId },
			headInfoIndex,
		});

		commitDiscard(
			{
				projectId,
				subjectCommitId: selection.commitId,
				dryRun: false,
			},
			{
				onSuccess: (response) => {
					let latestSelectionAfterDiscard = selectionAfterDiscard;

					rewrite: if (selectionAfterDiscard?._tag === "Commit") {
						const newId = response.workspace.replacedCommits[selectionAfterDiscard.commitId];
						if (newId === undefined) break rewrite;

						latestSelectionAfterDiscard = commitOperand({
							commitId: newId,
							changeId: selectionAfterDiscard.changeId,
						});
					}

					setCursor("stacks", latestSelectionAfterDiscard);
				},
			},
		);
	};

	const deleteSelectedBranchReference = () => {
		if (!selection || selection._tag !== "Branch") return;

		branchRemove({
			projectId,
			refName: selection.branchRef,
		});
	};

	const toggleFoldSelected = () => {
		if (foldableSegmentRef === null) return;

		toggleFoldedSegment(dispatch, {
			projectId,
			branchRefBytes: foldableSegmentRef.fullNameBytes,
			// A selected commit implies the segment is unfolded, so this is the
			// folding case and the selection needs the hand-off; a selected branch
			// row keeps its selection either way.
			select: selection?._tag === "Commit",
		});
	};

	const uncommitSelectedCommit = () => {
		if (!selection || selection._tag !== "Commit") return;

		const checkedCommitIds = projectSlice.selectors.selectCheckedCommitIds(
			store.getState(),
			projectId,
		);
		commitUncommit({
			projectId,
			assignTo: null,
			subjectCommitIds:
				checkedCommitIds.size > 0 ? Array.from(checkedCommitIds) : [selection.commitId],
			dryRun: false,
		});
	};

	const selectedSegmentIndex = selectionContext?.segmentIndex;

	const selectedPushContext =
		selectionStack && selectedSegmentIndex !== undefined
			? pushContextForSegment({
					segments: selectionStack.segments,
					segmentIndex: selectedSegmentIndex,
				})
			: null;
	const selectedStackRelativeTo = selectionStack ? stackBottomRelativeTo(selectionStack) : null;
	const selectedStackRebaseUpdate: BottomUpdate | null = selectedStackRelativeTo
		? { kind: "rebase", selector: selectedStackRelativeTo }
		: null;

	const pushSelectedBranch = () => {
		if (!selectedPushContext) return;

		const downstackPushStatus = downstackPushStatusFromSegments(
			selectedPushContext.downstackSegments,
		);

		workspaceBranchAndAncestorsPush({
			projectId,
			branch: decodeBytes(selectedPushContext.refName.fullNameBytes),
			withForce: downstackPushStatus.anyPushRequiresForce,
			skipForcePushProtection: false,
			runHooks: true,
			pushOpts: [],
		});
	};

	const updateSelectedStack = () => {
		if (selectedStackRebaseUpdate) {
			workspaceIntegrateUpstream({
				projectId,
				updates: [selectedStackRebaseUpdate],
				dryRun: false,
			});
		}
	};

	const openSelectedCommitInBrowser = async (): Promise<void> => {
		if (!selectedCommitForgeUrl) return;

		await window.lite.openInWebBrowser(selectedCommitForgeUrl.url);
	};

	const openSelectedBranchPRInBrowser = async (): Promise<void> => {
		if (selectedBranchPullRequestUrl === null) return;

		await window.lite.openInWebBrowser(selectedBranchPullRequestUrl);
	};

	const defaultOutlineHotkeysEnabled = isDefaultMode;
	const isSelectedCommit = selection?._tag === "Commit";
	const isSelectedBranch = selection?._tag === "Branch";
	const canPushSelectedBranch =
		!!selectedPushContext &&
		!isWorkspaceBranchAndAncestorsPushPending &&
		!downstackPushStatusDisabled(
			downstackPushStatusFromSegments(selectedPushContext.downstackSegments),
		);
	const canDeleteSelectedBranchReference =
		isSelectedBranch &&
		selectionStack !== undefined &&
		selectedSegmentIndex !== undefined &&
		canRemoveBranchReference(selectionStack, selectedSegmentIndex) &&
		!isBranchRemovePending;
	const canCheckCommits = useAppSelector((state) =>
		projectSlice.selectors.selectCanCheckCommits(state, projectId),
	);

	useNavigationIndexHotkeys({
		ref,
		navigationIndex,
		group: "Outline",
		select: (newItem) => setCursor("stacks", newItem),
		selection,
		onEdgeSpill,
		getKey: operandIdentityKey,
		operationSourcesForItem: (operand) => {
			const checkedOperands = projectSlice.selectors.selectCheckedOperands(
				store.getState(),
				projectId,
			);
			return checkedOperands.length > 0 ? checkedOperands : [operand];
		},
		selectSectionPredicate: (operand) => operand._tag === "Branch",
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
			hotkey: outlineHotkeys.composeCommitMessage.hotkey,
			callback: focusCommitMessageInput,
			options: {
				conflictBehavior: "allow",
			},
		},
		...Match.value(selection).pipe(
			Match.withReturnType<Array<UseHotkeyDefinition>>(),
			Match.tags({
				Commit: (selection): Array<UseHotkeyDefinition> => [
					{
						hotkey: outlineHotkeys.rewordCommit.hotkey,
						callback: () => {
							startRewordCommit(selection);
						},
						options: {
							conflictBehavior: "allow",
							enabled: defaultOutlineHotkeysEnabled,
							target: ref,
							meta: outlineHotkeys.rewordCommit.meta,
						},
					},
					{
						hotkey: "F2",
						callback: () => {
							startRewordCommit(selection);
						},
						options: {
							conflictBehavior: "allow",
							enabled: defaultOutlineHotkeysEnabled,
							target: ref,
						},
					},
				],
				Branch: (selection): Array<UseHotkeyDefinition> => [
					{
						hotkey: outlineHotkeys.renameBranch.hotkey,
						callback: () => {
							startRenameBranch(selection);
						},
						options: {
							conflictBehavior: "allow",
							enabled: defaultOutlineHotkeysEnabled,
							target: ref,
							meta: outlineHotkeys.renameBranch.meta,
						},
					},
					{
						hotkey: "F2",
						callback: () => {
							startRenameBranch(selection);
						},
						options: {
							conflictBehavior: "allow",
							enabled: defaultOutlineHotkeysEnabled,
							target: ref,
						},
					},
				],
			}),
			Match.orElse(() => []),
		),
		{
			hotkey: outlineHotkeys.checkCommit.hotkey,
			callback: toggleSelectedCommitChecked,
			options: {
				conflictBehavior: "allow",
				enabled: defaultOutlineHotkeysEnabled && isSelectedCommit && canCheckCommits,
				preventDefault: false,
				stopPropagation: false,
				target: ref,
				meta: outlineHotkeys.checkCommit.meta,
			},
		},
		{
			hotkey: "Shift+Space",
			callback: toggleSelectedCommitChecked,
			options: {
				conflictBehavior: "allow",
				enabled: defaultOutlineHotkeysEnabled && isSelectedCommit && canCheckCommits,
				preventDefault: false,
				stopPropagation: false,
				target: ref,
			},
		},
		{
			hotkey: outlineHotkeys.checkBranchCommits.hotkey,
			callback: toggleSelectedBranchChecked,
			options: {
				conflictBehavior: "allow",
				enabled: defaultOutlineHotkeysEnabled && isSelectedBranch && canCheckCommits,
				target: ref,
				meta: outlineHotkeys.checkBranchCommits.meta,
			},
		},
		{
			hotkey: outlineHotkeys.deleteBranchRef.hotkey,
			callback: deleteSelectedBranchReference,
			options: {
				conflictBehavior: "allow",
				enabled: defaultOutlineHotkeysEnabled && canDeleteSelectedBranchReference,
				target: ref,
				meta: outlineHotkeys.deleteBranchRef.meta,
			},
		},
		{
			hotkey: outlineHotkeys.deleteCommit.hotkey,
			callback: deleteSelectedCommit,
			options: {
				conflictBehavior: "allow",
				enabled: defaultOutlineHotkeysEnabled && isSelectedCommit && !isCommitDiscardPending,
				target: ref,
				meta: outlineHotkeys.deleteCommit.meta,
			},
		},
		{
			hotkey: outlineHotkeys.uncommitCommit.hotkey,
			callback: uncommitSelectedCommit,
			options: {
				conflictBehavior: "allow",
				enabled: defaultOutlineHotkeysEnabled && isSelectedCommit && !isCommitUncommitPending,
				target: ref,
				meta: outlineHotkeys.uncommitCommit.meta,
			},
		},
		{
			hotkey: outlineHotkeys.toggleFoldBranch.hotkey,
			callback: toggleFoldSelected,
			options: {
				conflictBehavior: "allow",
				enabled: defaultOutlineHotkeysEnabled && foldableSegmentRef !== null,
				target: ref,
				meta: outlineHotkeys.toggleFoldBranch.meta,
			},
		},
		{
			hotkey: outlineHotkeys.openCommitInBrowser.hotkey,
			callback: () => void openSelectedCommitInBrowser(),
			options: {
				conflictBehavior: "allow",
				enabled: defaultOutlineHotkeysEnabled && isSelectedCommit && !!selectedCommitForgeUrl,
				target: ref,
				meta: outlineHotkeys.openCommitInBrowser.meta,
			},
		},
		{
			hotkey: outlineHotkeys.moveCommitUp.hotkey,
			callback: () => moveSelectedCommit(-1),
			options: {
				conflictBehavior: "allow",
				enabled: defaultOutlineHotkeysEnabled && isSelectedCommit && !isCommitMovePending,
				target: ref,
				meta: outlineHotkeys.moveCommitUp.meta,
			},
		},
		{
			hotkey: outlineHotkeys.moveCommitDown.hotkey,
			callback: () => moveSelectedCommit(1),
			options: {
				conflictBehavior: "allow",
				enabled: defaultOutlineHotkeysEnabled && isSelectedCommit && !isCommitMovePending,
				target: ref,
				meta: outlineHotkeys.moveCommitDown.meta,
			},
		},
		{
			hotkey: outlineHotkeys.workspaceBranchAndAncestorsPush.hotkey,
			callback: pushSelectedBranch,
			options: {
				conflictBehavior: "allow",
				enabled: defaultOutlineHotkeysEnabled && canPushSelectedBranch,
				target: ref,
				meta: outlineHotkeys.workspaceBranchAndAncestorsPush.meta,
			},
		},
		{
			hotkey: outlineHotkeys.openPRInBrowser.hotkey,
			callback: () => void openSelectedBranchPRInBrowser(),
			options: {
				conflictBehavior: "allow",
				enabled:
					defaultOutlineHotkeysEnabled && isSelectedBranch && selectedBranchPullRequestUrl !== null,
				target: ref,
				meta: outlineHotkeys.openPRInBrowser.meta,
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
					!isWorkspaceIntegrateUpstreamPending,
				target: ref,
				meta: outlineHotkeys.updateStack.meta,
			},
		},
		{
			hotkey: outlineHotkeys.insertEmptyCommitAbove.hotkey,
			callback: () => insertEmptyCommit("above"),
			options: {
				conflictBehavior: "allow",
				enabled:
					defaultOutlineHotkeysEnabled &&
					(isSelectedBranch || isSelectedCommit) &&
					!isCommitInsertBlankPending,
				target: ref,
				meta: outlineHotkeys.insertEmptyCommitAbove.meta,
			},
		},
		{
			hotkey: outlineHotkeys.insertEmptyCommitBelow.hotkey,
			callback: () => insertEmptyCommit("below"),
			options: {
				conflictBehavior: "allow",
				enabled:
					defaultOutlineHotkeysEnabled &&
					(isSelectedBranch || isSelectedCommit) &&
					!isCommitInsertBlankPending,
				target: ref,
				meta: outlineHotkeys.insertEmptyCommitBelow.meta,
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
						]
					: [],
		),
	]);
};
