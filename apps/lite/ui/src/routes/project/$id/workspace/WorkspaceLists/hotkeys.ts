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
import { startKeyboardTransfer, setCursor, startInlineEdit, useSelection } from "#ui/use-cursor.ts";
import { forgeInfoOptions, headInfoQueryOptions } from "#ui/api/queries.ts";
import { decodeBytes } from "#ui/api/bytes.ts";
import { getHeadInfoIndex } from "#ui/api/ref-info.ts";
import { commitForgeUrl } from "#ui/commit.ts";
import { sidebarHotkeys } from "#ui/hotkeys.ts";
import { branchAddress, commitAddress, addressIdentityKey, type Address } from "#ui/addresses.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { interfaceSlice } from "#ui/interface/state.ts";
import { focusScope, useAddressSpaceHotkeys } from "#ui/focus-scopes.ts";
import { useAppDispatch, useAppSelector, useAppStore } from "#ui/store.ts";
import type { AddressSpace } from "#ui/workspace/address-space.ts";
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
import { selectAfterDiscardedCommits } from "./selectAfterDiscardedCommit.ts";
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

export const useActiveListsHotkeys = ({
	addressSpace,
	projectId,
	ref,
	checkCommit,
	focusCommitMessageInput,
	onEdgeSpill,
}: {
	addressSpace: AddressSpace<Address>;
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
	const selection = useSelection("applied", addressSpace);
	const noOperationPending = useAppSelector(
		(state) => projectSlice.selectors.selectPendingOperation(state, projectId)._tag === "None",
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
					setCursor("applied", branchAddress({ branchRef: response.newRef.fullNameBytes }));
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
			projectSlice.actions.checkAddresses({
				projectId,
				addresses: selectedBranchSegment.commits.map((commit) =>
					commitAddress({ commitId: commit.id, changeId: commit.changeId }),
				),
				checked: !selectedBranchCommitsChecked,
			}),
		);
	};

	const moveSelectedCommit = (offset: -1 | 1) => {
		if (!selection || selection._tag !== "Commit") return;

		const source = commitAddress(selection);
		const selectionIdx = addressSpace.indexByKey.get(addressIdentityKey(source));
		if (selectionIdx === undefined) return;

		const checkedCommitIds = projectSlice.selectors.selectCheckedCommitIds(
			store.getState(),
			projectId,
		);
		const subjectCommitIds =
			checkedCommitIds.size > 0 ? checkedCommitIds : new Set([selection.commitId]);

		let nextItemIndex = selectionIdx;
		let nextItem: Address | undefined;
		do {
			nextItemIndex += offset;
			nextItem = addressSpace.items[nextItemIndex];
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
				throw new Error("Only commits and branches are valid sidebar items");
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
		const checkedCommitIds = projectSlice.selectors.selectCheckedCommitIds(
			store.getState(),
			projectId,
		);
		const subjectCommitIds =
			checkedCommitIds.size > 0 ? checkedCommitIds : new Set([selection.commitId]);

		const selectionAfterDiscard = selectAfterDiscardedCommits({
			addressSpace,
			commit: { commitId: selection.commitId, changeId: selection.changeId },
			discardedCommitIds: subjectCommitIds,
			headInfoIndex,
		});

		commitDiscard(
			{
				projectId,
				subjectCommitIds: Array.from(subjectCommitIds),
				dryRun: false,
			},
			{
				onSuccess: (response) => {
					let latestSelectionAfterDiscard = selectionAfterDiscard;

					rewrite: if (selectionAfterDiscard?._tag === "Commit") {
						const newId = response.workspace.replacedCommits[selectionAfterDiscard.commitId];
						if (newId === undefined) break rewrite;

						latestSelectionAfterDiscard = commitAddress({
							commitId: newId,
							changeId: selectionAfterDiscard.changeId,
						});
					}

					setCursor("applied", latestSelectionAfterDiscard);
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

	const defaultSidebarHotkeysEnabled = noOperationPending;
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
	const operationSourcesForItem = (address: Address): Array<Address> => {
		const checkedAddresses = projectSlice.selectors.selectCheckedAddresses(
			store.getState(),
			projectId,
		);
		return checkedAddresses.length > 0 ? checkedAddresses : [address];
	};

	useAddressSpaceHotkeys({
		projectId,
		ref,
		addressSpace,
		group: "Sidebar",
		select: (newItem) => setCursor("applied", newItem),
		selection,
		onEdgeSpill,
		getKey: addressIdentityKey,
		operationSourcesForItem,
		selectSectionPredicate: (address) => address._tag === "Branch",
	});

	useHotkeys([
		{
			hotkey: sidebarHotkeys.selectBranch.hotkey,
			callback: openBranchPicker,
			options: {
				conflictBehavior: "allow",
				meta: sidebarHotkeys.selectBranch.meta,
			},
		},
		{
			hotkey: sidebarHotkeys.composeCommitMessage.hotkey,
			callback: focusCommitMessageInput,
			options: {
				conflictBehavior: "allow",
			},
		},
		{
			hotkey: sidebarHotkeys.copy.hotkey,
			callback: () => {
				if (selection === null) return;

				const sources = operationSourcesForItem(selection);
				if (!sources.every((source) => source._tag === "Commit")) return;

				startKeyboardTransfer({
					sources,
					kind: "copy",
					placement: "above",
				});
				focusScope("sidebar");
			},
			options: {
				conflictBehavior: "allow",
				enabled: defaultSidebarHotkeysEnabled && isSelectedCommit,
				ignoreInputs: true,
				target: ref,
				meta: sidebarHotkeys.copy.meta,
			},
		},
		...Match.value(selection).pipe(
			Match.withReturnType<Array<UseHotkeyDefinition>>(),
			Match.tags({
				Commit: (selection): Array<UseHotkeyDefinition> => [
					{
						hotkey: sidebarHotkeys.rewordCommit.hotkey,
						callback: () => {
							startInlineEdit(selection);
						},
						options: {
							conflictBehavior: "allow",
							enabled: defaultSidebarHotkeysEnabled,
							target: ref,
							meta: sidebarHotkeys.rewordCommit.meta,
						},
					},
					{
						hotkey: "F2",
						callback: () => {
							startInlineEdit(selection);
						},
						options: {
							conflictBehavior: "allow",
							enabled: defaultSidebarHotkeysEnabled,
							target: ref,
						},
					},
				],
				Branch: (selection): Array<UseHotkeyDefinition> => [
					{
						hotkey: sidebarHotkeys.renameBranch.hotkey,
						callback: () => {
							startInlineEdit(selection);
						},
						options: {
							conflictBehavior: "allow",
							enabled: defaultSidebarHotkeysEnabled,
							target: ref,
							meta: sidebarHotkeys.renameBranch.meta,
						},
					},
					{
						hotkey: "F2",
						callback: () => {
							startInlineEdit(selection);
						},
						options: {
							conflictBehavior: "allow",
							enabled: defaultSidebarHotkeysEnabled,
							target: ref,
						},
					},
				],
			}),
			Match.orElse(() => []),
		),
		{
			hotkey: sidebarHotkeys.checkCommit.hotkey,
			callback: toggleSelectedCommitChecked,
			options: {
				conflictBehavior: "allow",
				enabled: defaultSidebarHotkeysEnabled && isSelectedCommit && canCheckCommits,
				preventDefault: false,
				stopPropagation: false,
				target: ref,
				meta: sidebarHotkeys.checkCommit.meta,
			},
		},
		{
			hotkey: "Shift+Space",
			callback: toggleSelectedCommitChecked,
			options: {
				conflictBehavior: "allow",
				enabled: defaultSidebarHotkeysEnabled && isSelectedCommit && canCheckCommits,
				preventDefault: false,
				stopPropagation: false,
				target: ref,
			},
		},
		{
			hotkey: sidebarHotkeys.checkBranchCommits.hotkey,
			callback: toggleSelectedBranchChecked,
			options: {
				conflictBehavior: "allow",
				enabled: defaultSidebarHotkeysEnabled && isSelectedBranch && canCheckCommits,
				target: ref,
				meta: sidebarHotkeys.checkBranchCommits.meta,
			},
		},
		{
			hotkey: sidebarHotkeys.deleteBranchRef.hotkey,
			callback: deleteSelectedBranchReference,
			options: {
				conflictBehavior: "allow",
				enabled: defaultSidebarHotkeysEnabled && canDeleteSelectedBranchReference,
				target: ref,
				meta: sidebarHotkeys.deleteBranchRef.meta,
			},
		},
		{
			hotkey: sidebarHotkeys.deleteCommit.hotkey,
			callback: deleteSelectedCommit,
			options: {
				conflictBehavior: "allow",
				enabled: defaultSidebarHotkeysEnabled && isSelectedCommit && !isCommitDiscardPending,
				target: ref,
				meta: sidebarHotkeys.deleteCommit.meta,
			},
		},
		{
			hotkey: sidebarHotkeys.uncommitCommit.hotkey,
			callback: uncommitSelectedCommit,
			options: {
				conflictBehavior: "allow",
				enabled: defaultSidebarHotkeysEnabled && isSelectedCommit && !isCommitUncommitPending,
				target: ref,
				meta: sidebarHotkeys.uncommitCommit.meta,
			},
		},
		{
			hotkey: sidebarHotkeys.toggleFoldBranch.hotkey,
			callback: toggleFoldSelected,
			options: {
				conflictBehavior: "allow",
				enabled: defaultSidebarHotkeysEnabled && foldableSegmentRef !== null,
				target: ref,
				meta: sidebarHotkeys.toggleFoldBranch.meta,
			},
		},
		{
			hotkey: sidebarHotkeys.openCommitInBrowser.hotkey,
			callback: () => void openSelectedCommitInBrowser(),
			options: {
				conflictBehavior: "allow",
				enabled: defaultSidebarHotkeysEnabled && isSelectedCommit && !!selectedCommitForgeUrl,
				target: ref,
				meta: sidebarHotkeys.openCommitInBrowser.meta,
			},
		},
		{
			hotkey: sidebarHotkeys.moveCommitUp.hotkey,
			callback: () => moveSelectedCommit(-1),
			options: {
				conflictBehavior: "allow",
				enabled: defaultSidebarHotkeysEnabled && isSelectedCommit && !isCommitMovePending,
				target: ref,
				meta: sidebarHotkeys.moveCommitUp.meta,
			},
		},
		{
			hotkey: sidebarHotkeys.moveCommitDown.hotkey,
			callback: () => moveSelectedCommit(1),
			options: {
				conflictBehavior: "allow",
				enabled: defaultSidebarHotkeysEnabled && isSelectedCommit && !isCommitMovePending,
				target: ref,
				meta: sidebarHotkeys.moveCommitDown.meta,
			},
		},
		{
			hotkey: sidebarHotkeys.workspaceBranchAndAncestorsPush.hotkey,
			callback: pushSelectedBranch,
			options: {
				conflictBehavior: "allow",
				enabled: defaultSidebarHotkeysEnabled && canPushSelectedBranch,
				target: ref,
				meta: sidebarHotkeys.workspaceBranchAndAncestorsPush.meta,
			},
		},
		{
			hotkey: sidebarHotkeys.openPRInBrowser.hotkey,
			callback: () => void openSelectedBranchPRInBrowser(),
			options: {
				conflictBehavior: "allow",
				enabled:
					defaultSidebarHotkeysEnabled && isSelectedBranch && selectedBranchPullRequestUrl !== null,
				target: ref,
				meta: sidebarHotkeys.openPRInBrowser.meta,
			},
		},
		{
			hotkey: sidebarHotkeys.updateStack.hotkey,
			callback: updateSelectedStack,
			options: {
				conflictBehavior: "allow",
				enabled:
					defaultSidebarHotkeysEnabled &&
					!!selectedStackRebaseUpdate &&
					!isWorkspaceIntegrateUpstreamPending,
				target: ref,
				meta: sidebarHotkeys.updateStack.meta,
			},
		},
		{
			hotkey: sidebarHotkeys.insertEmptyCommitAbove.hotkey,
			callback: () => insertEmptyCommit("above"),
			options: {
				conflictBehavior: "allow",
				enabled:
					defaultSidebarHotkeysEnabled &&
					(isSelectedBranch || isSelectedCommit) &&
					!isCommitInsertBlankPending,
				target: ref,
				meta: sidebarHotkeys.insertEmptyCommitAbove.meta,
			},
		},
		{
			hotkey: sidebarHotkeys.insertEmptyCommitBelow.hotkey,
			callback: () => insertEmptyCommit("below"),
			options: {
				conflictBehavior: "allow",
				enabled:
					defaultSidebarHotkeysEnabled &&
					(isSelectedBranch || isSelectedCommit) &&
					!isCommitInsertBlankPending,
				target: ref,
				meta: sidebarHotkeys.insertEmptyCommitBelow.meta,
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
								hotkey: sidebarHotkeys.createDependentBranchAbove.hotkey,
								callback: () => createDependentBranchAbove(relativeTo),
								options: {
									conflictBehavior: "allow",
									enabled: defaultSidebarHotkeysEnabled,
									target: ref,
									meta: sidebarHotkeys.createDependentBranchAbove.meta,
									requireReset: true,
								},
							} satisfies UseHotkeyDefinition,
						]
					: [],
		),
	]);
};
