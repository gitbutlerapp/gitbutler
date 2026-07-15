import {
	useBranchCreate,
	useCommitAmend,
	useCommitDiscard,
	useCommitInsertBlank,
	useCommitMove,
	useWorkspaceBranchAndAncestorsPush,
	useWorkspaceIntegrateUpstream,
} from "#ui/api/mutations.ts";
import { forgeInfoOptions, headInfoQueryOptions } from "#ui/api/queries.ts";
import { decodeBytes } from "#ui/api/bytes.ts";
import { getHeadInfoIndex } from "#ui/api/ref-info.ts";
import { commitForgeUrl, rewrittenCommitSelection } from "#ui/commit.ts";
import { outlineHotkeys } from "#ui/hotkeys.ts";
import {
	branchOperand,
	commitOperand,
	operandIdentityKey,
	uncommittedChangesOperand,
	type Operand,
} from "#ui/operands.ts";
import { focusSelectionScope, useNavigationIndexHotkeys } from "#ui/selection-scopes.ts";
import { useProjectStore } from "#ui/store.ts";
import { type NavigationIndex } from "#ui/workspace/navigation-index.ts";
import { prForgeUrl } from "#ui/pr.ts";
import { stackBottomRelativeTo } from "#ui/api/stack.ts";
import {
	AbsorptionTarget,
	BranchReference,
	BottomUpdate,
	InsertSide,
	RelativeTo,
	Segment,
} from "@gitbutler/but-sdk";
import { UseHotkeyDefinition, useHotkeys } from "@tanstack/react-hotkeys";
import { useQuery } from "@tanstack/react-query";
import { Match } from "effect";
import { type RefObject } from "react";
import { commitMessageInputId } from "../CommitForm.tsx";
import { selectAfterDiscardedCommit } from "./selectAfterDiscardedCommit.ts";
import { downstackPushStatusDisabled, downstackPushStatusFromSegments } from "#ui/segment.ts";

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

const focusCommitMessageInput = () => {
	document.getElementById(commitMessageInputId)?.focus();
};

export const useOutlineTreeHotkeys = ({
	navigationIndex,
	projectId,
	ref,
}: {
	navigationIndex: NavigationIndex<Operand>;
	projectId: string;
	ref: RefObject<HTMLElement | null>;
}) => {
	const { data: headInfoIndex } = useQuery({
		...headInfoQueryOptions(projectId),
		select: getHeadInfoIndex,
	});
	const { data: forgeInfo } = useQuery(forgeInfoOptions(projectId));
	const projectStore = useProjectStore(projectId);
	const selection = projectStore.selectedOutline(navigationIndex);
	const isDefaultMode = projectStore.outlineMode._tag === "Default";

	const selectedStack =
		selection && "stackId" in selection
			? headInfoIndex?.stackContextById(selection.stackId)?.stack
			: undefined;
	const selectedBranchSegment =
		selection?._tag === "Branch"
			? headInfoIndex?.branchContextByRefBytes(selection.branchRef)?.segment
			: undefined;

	const selectedBranchCommitsChecked =
		selectedBranchSegment && selectedBranchSegment.commits.length > 0
			? selectedBranchSegment.commits.every((commit) => projectStore.isCommitChecked(commit.id))
			: false;
	const selectedCommitChecked =
		selection?._tag === "Commit" ? projectStore.isCommitChecked(selection.commitId) : false;
	const selectedCommit =
		selection?._tag === "Commit"
			? (headInfoIndex?.commitContextById(selection.commitId) ?? null)?.commit
			: null;
	const selectedCommitForgeUrl =
		selectedCommit && forgeInfo ? commitForgeUrl(selectedCommit, forgeInfo) : null;
	const selectedBranchPullRequest = selectedBranchSegment?.metadata?.review.pullRequest ?? null;
	const selectedBranchPullRequestUrl =
		selectedBranchPullRequest !== null && forgeInfo
			? prForgeUrl(selectedBranchPullRequest, forgeInfo)
			: null;

	const { isPending: isCommitMovePending, mutate: commitMove } = useCommitMove();
	const { isPending: isCommitDiscardPending, mutate: commitDiscard } = useCommitDiscard();
	const { isPending: isCommitInsertBlankPending, mutate: commitInsertBlank } =
		useCommitInsertBlank();
	const { isPending: isCommitAmendPending, mutate: commitAmend } = useCommitAmend({
		projectId,
	});
	const {
		isPending: isWorkspaceBranchAndAncestorsPushPending,
		mutate: workspaceBranchAndAncestorsPush,
	} = useWorkspaceBranchAndAncestorsPush();
	const { isPending: isWorkspaceIntegrateUpstreamPending, mutate: workspaceIntegrateUpstream } =
		useWorkspaceIntegrateUpstream();
	const { mutate: branchCreate } = useBranchCreate();

	const openBranchPicker = () => {
		projectStore.openDialog({ _tag: "BranchPicker" });
	};

	const enterAbsorbMode = (source: Operand, sourceTarget: AbsorptionTarget) => {
		projectStore.enterAbsorbMode(source, sourceTarget);
	};

	const amendCommit = () => {
		if (selection?._tag !== "Commit") return;

		commitAmend({ commitId: selection.commitId });
	};

	const setCommitTarget = (relativeTo: RelativeTo) => {
		projectStore.setCommitTarget(relativeTo);
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
					const newBranchStack = getHeadInfoIndex(
						response.workspace.headInfo,
					).branchContextByRefBytes(response.newRef.fullNameBytes)?.stack;

					if (newBranchStack && newBranchStack.id !== null) {
						projectStore.selectOutline(
							branchOperand({
								stackId: newBranchStack.id,
								branchRef: response.newRef.fullNameBytes,
							}),
						);
					}
				},
			},
		);
	};

	const toggleSelectedCommitChecked = () => {
		if (!selection || selection._tag !== "Commit") return;

		projectStore.setCommitChecked(selection.commitId, !selectedCommitChecked);
	};

	const toggleSelectedBranchChecked = () => {
		if (!selectedBranchSegment) return;

		projectStore.setCommitsChecked(
			selectedBranchSegment.commits.map((commit) => commit.id),
			!selectedBranchCommitsChecked,
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

		commitMove({
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

		commitDiscard(
			{
				projectId,
				subjectCommitId: selection.commitId,
				dryRun: false,
			},
			{
				onSuccess: (response) => {
					projectStore.selectOutline(
						rewrittenCommitSelection({
							selection: selectionAfterDiscard,
							replacedCommits: response.workspace.replacedCommits,
							headInfo: response.workspace.headInfo,
						}),
					);
				},
			},
		);
	};

	const selectedSegmentIndex =
		selection?._tag === "Branch"
			? headInfoIndex?.branchContextByRefBytes(selection.branchRef)?.segmentIndex
			: selection?._tag === "Commit"
				? headInfoIndex?.commitContextById(selection.commitId)?.segmentIndex
				: undefined;

	const selectedPushContext =
		selectedStack && selectedSegmentIndex !== undefined
			? pushContextForSegment({
					segments: selectedStack.segments,
					segmentIndex: selectedSegmentIndex,
				})
			: null;
	const selectedStackRelativeTo = selectedStack ? stackBottomRelativeTo(selectedStack) : null;
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
	const isSelectedChanges = selection?._tag === "UncommittedChanges";
	const canPushSelectedBranch =
		!!selectedPushContext &&
		!isWorkspaceBranchAndAncestorsPushPending &&
		!downstackPushStatusDisabled(
			downstackPushStatusFromSegments(selectedPushContext.downstackSegments),
		);

	useNavigationIndexHotkeys({
		ref,
		navigationIndex,
		projectId,
		group: "Outline",
		select: (selection) => projectStore.selectOutline(selection),
		selection,
		getKey: operandIdentityKey,
		operationSourceForItem: (operand) => operand,
		selectSectionPredicate: (operand) =>
			operand._tag === "Branch" || operand._tag === "UncommittedChanges",
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
				projectStore.selectOutline(uncommittedChangesOperand);
				focusSelectionScope("outline");
			},
			options: { conflictBehavior: "allow" },
		},
		{
			hotkey: outlineHotkeys.composeCommitMessage.hotkey,
			callback: () => {
				focusCommitMessageInput();
			},
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
							projectStore.startRewordCommit(selection);
						},
						options: {
							conflictBehavior: "allow",
							enabled: defaultOutlineHotkeysEnabled,
							target: ref,
							meta: outlineHotkeys.rewordCommit.meta,
						},
					},
				],
				Branch: (selection): Array<UseHotkeyDefinition> => [
					{
						hotkey: outlineHotkeys.renameBranch.hotkey,
						callback: () => {
							projectStore.startRenameBranch(selection);
						},
						options: {
							conflictBehavior: "allow",
							enabled: defaultOutlineHotkeysEnabled,
							target: ref,
							meta: outlineHotkeys.renameBranch.meta,
						},
					},
				],
				UncommittedChanges: (): Array<UseHotkeyDefinition> => [
					{
						hotkey: outlineHotkeys.composeCommitMessageFromChanges.hotkey,
						callback: focusCommitMessageInput,
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
			hotkey: outlineHotkeys.amendCommit.hotkey,
			callback: amendCommit,
			options: {
				conflictBehavior: "allow",
				enabled: defaultOutlineHotkeysEnabled && isSelectedCommit && !isCommitAmendPending,
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
				enabled: defaultOutlineHotkeysEnabled && isSelectedCommit && !isCommitDiscardPending,
				target: ref,
				meta: outlineHotkeys.deleteCommit.meta,
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
			hotkey: outlineHotkeys.insertEmptyCommit.hotkey,
			callback: insertEmptyCommit,
			options: {
				conflictBehavior: "allow",
				enabled:
					defaultOutlineHotkeysEnabled &&
					(isSelectedBranch || isSelectedCommit) &&
					!isCommitInsertBlankPending,
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
				enterAbsorbMode(uncommittedChangesOperand, { type: "all" });
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
