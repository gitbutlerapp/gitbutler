import {
	branchFileParent,
	branchAddress,
	commitFileParent,
	commitAddress,
	hunkAddress,
	fileAddress,
	addressEquals,
	addressIdentityKey,
	type BranchAddress,
	type CommitAddress,
	type FileAddress,
	type FileParent,
	type HunkAddress,
	type Address,
} from "#ui/addresses.ts";
import type { Placement, TransferKind } from "#ui/operations/operation.ts";
import {
	pendingAbsorb,
	noPendingOperation,
	pendingInlineEdit,
	keyboardTransfer,
	pointerTransfer,
	pendingTransfer,
	type PendingInlineEdit,
	type PendingOperation,
	type PendingTransfer,
} from "#ui/operations/pending-operation.ts";
import {
	cursorKey,
	remapDiffCursor,
	remapDiffCursorBranch,
	type DiffLineSelection,
	type WorkspaceCursorSnapshot,
} from "#ui/cursors.ts";
import { createSelector } from "@reduxjs/toolkit";
import type { AbsorptionTarget } from "@gitbutler/but-sdk";
import { Match } from "effect";
import {
	branchesReducers,
	createInitialBranchesState,
	getBranchesSelectors,
	type BranchFilter,
	type BranchesState,
} from "./branches.ts";
import { decodeBytes } from "#ui/api/bytes.ts";
import type { FocusScope } from "#ui/focus-scopes.ts";
import {
	createInitialUpstreamState,
	getUpstreamSelectors,
	upstreamReducers,
	type UpstreamState,
} from "./upstream.ts";

/** The workspace page's two lists; the one named here is active and drives the details pane. */
export type ActiveList = "applied" | "uncommitted";

export type CheckableAddress = Extract<Address, { _tag: "Commit" | "File" | "Hunk" }>;

export type BranchTab = "diff" | "pr";

/**
 * A conflict checked for a batch resolution. Ids survive the rewrites that
 * compact hunk positions, so checks carry across; the commit id is remapped.
 */
export type CheckedConflict = { commitId: string; path: string; id: string };

const conflictCheckKey = ({ commitId, path, id }: CheckedConflict): string =>
	`${commitId}\u0000${path}\u0000${id}`;

type WorkspaceState = {
	checkedAddresses: Record<string, CheckableAddress>;
	checkedConflicts: Record<string, CheckedConflict>;
	/**
	 * Branch segments whose commits are hidden, keyed by full ref name.
	 *
	 * Folded rather than unfolded, the inverse of the branches tab: the
	 * workspace is the working view, so its commits show by default and it is
	 * hiding them that is the exception worth recording.
	 */
	foldedSegments: Record<string, true>;
	dependencyCommitIds: Array<string>;
	pendingOperation: PendingOperation;
	/**
	 * What became of the last operation that ended without one, stated where the operation's own
	 * controls stood. An operation that cannot run must not hold the workspace open waiting to be
	 * aimed, so it clears itself and leaves this behind to say why.
	 */
	notice: string | null;
	selectedBranchTabs: Record<string, BranchTab>;
	/**
	 * The diff cursor. Its five siblings live in the URL (use-cursor.ts); this
	 * one holds an exact visual line range in Redux instead of a URL query param.
	 */
	diffCursor: DiffLineSelection | null;
	/**
	 * File filter queries, or `null` while a filter is closed. An open but empty
	 * filter is not the same as a closed one: it keeps the input in place and the
	 * list unnarrowed.
	 *
	 * The sidebar's uncommitted list and the details pane's file list filter
	 * independently, and can both be open at once.
	 */
	uncommittedFilesFilter: string | null;
	filesFilter: string | null;
	/**
	 * Whether the uncommitted list is ordered by file modification time, newest
	 * first. Order only: the shared list/tree display mode stays in force.
	 */
	uncommittedFilesRecentFirst: boolean;
	/**
	 * Directories whose contents the tree view hides, keyed by directory path.
	 *
	 * Collapsed rather than expanded, as with {@link WorkspaceState.foldedSegments}:
	 * a tree opens showing everything, so it is the hiding that is worth recording.
	 * One set per list, as with the filters — the two lists hold different files
	 * and each is somewhere the user is looking separately.
	 */
	uncommittedFilesCollapsedDirectories: Record<string, true>;
	filesCollapsedDirectories: Record<string, true>;
};

const createInitialWorkspaceState = (): WorkspaceState => ({
	checkedAddresses: {},
	checkedConflicts: {},
	foldedSegments: {},
	dependencyCommitIds: [],
	pendingOperation: noPendingOperation,
	notice: null,
	selectedBranchTabs: {},
	diffCursor: null,
	uncommittedFilesFilter: null,
	filesFilter: null,
	uncommittedFilesRecentFirst: false,
	uncommittedFilesCollapsedDirectories: {},
	filesCollapsedDirectories: {},
});

export type PageId = "workspace" | "upstream" | "branches";

export type ProjectState = {
	filesVisible: boolean;
	branches: BranchesState;
	upstream: UpstreamState;
	workspace: WorkspaceState;
};

export const createInitialProjectState = (): ProjectState => ({
	filesVisible: true,
	branches: createInitialBranchesState(),
	upstream: createInitialUpstreamState(),
	workspace: createInitialWorkspaceState(),
});

export const projectReducers = {
	selectDiffCursor: (
		state: ProjectState,
		{ selection }: { selection: DiffLineSelection | null },
	) => {
		const current = state.workspace.diffCursor;
		if (
			selection !== null &&
			current !== null &&
			cursorKey.diff(current) === cursorKey.diff(selection)
		)
			return;

		state.workspace.diffCursor = selection;
	},
	toggleUpstreamSegment: (state: ProjectState, { segmentId }: { segmentId: string }) => {
		upstreamReducers.toggleSegment(state.upstream, { segmentId });
	},
	startInlineEdit: (state: ProjectState, edit: PendingInlineEdit) => {
		state.workspace.pendingOperation = pendingInlineEdit(edit);
		state.workspace.notice = null;
	},
	updateRewrittenBranchReferences: (
		state: ProjectState,
		{ oldBranch, newBranch }: { oldBranch: BranchAddress; newBranch: BranchAddress },
	) => {
		const workspaceState = state.workspace;
		const oldBranchAddress = branchAddress(oldBranch);

		if (workspaceState.diffCursor) {
			workspaceState.diffCursor = remapDiffCursorBranch(
				workspaceState.diffCursor,
				oldBranch,
				newBranch,
			);
		}

		if (
			workspaceState.pendingOperation._tag === "InlineEdit" &&
			workspaceState.pendingOperation.address._tag === "Branch" &&
			addressEquals(workspaceState.pendingOperation.address, oldBranchAddress)
		)
			workspaceState.pendingOperation = pendingInlineEdit({ address: branchAddress(newBranch) });

		const oldFileParent = branchFileParent(oldBranch);
		const newFileParent = branchFileParent(newBranch);
		for (const [key, address] of Object.entries(workspaceState.checkedAddresses)) {
			if (
				address._tag !== "File" ||
				address.parent._tag !== "Branch" ||
				!addressEquals(address.parent, oldFileParent)
			)
				continue;

			const newAddress = fileAddress({ parent: newFileParent, path: address.path });
			delete workspaceState.checkedAddresses[key];
			workspaceState.checkedAddresses[addressIdentityKey(newAddress)] = newAddress;
		}
	},
	startTransfer: (state: ProjectState, { transfer }: { transfer: PendingTransfer }) => {
		state.workspace.pendingOperation = pendingTransfer(transfer);
		state.workspace.notice = null;
	},
	startKeyboardTransfer: (
		state: ProjectState,
		{
			sources,
			kind,
			placement,
			restoreSelection,
			restoreFocus,
		}: {
			sources: Array<Address>;
			kind: TransferKind;
			placement?: Placement;
			restoreSelection: WorkspaceCursorSnapshot;
			restoreFocus: FocusScope | null;
		},
	) => {
		state.workspace.pendingOperation = pendingTransfer(
			keyboardTransfer({
				sources,
				kind,
				placement: placement ?? "into",
				restoreSelection,
				restoreFocus,
			}),
		);
		state.workspace.notice = null;
	},
	startAbsorb: (
		state: ProjectState,
		{
			sources,
			sourceTarget,
			restoreSelection,
		}: {
			sources: Array<Address>;
			sourceTarget: AbsorptionTarget;
			restoreSelection: WorkspaceCursorSnapshot;
		},
	) => {
		state.workspace.pendingOperation = pendingAbsorb({ sources, restoreSelection, sourceTarget });
		state.workspace.notice = null;
	},
	updatePointerTransfer: (
		state: ProjectState,
		{ target, placement }: { target: Address | null; placement: Placement | null },
	) => {
		const workspaceState = state.workspace;
		Match.value(workspaceState.pendingOperation).pipe(
			Match.when({ _tag: "Transfer", value: { _tag: "Pointer" } }, ({ value: transfer }) => {
				const sameTarget =
					target === null
						? transfer.target === null
						: transfer.target !== null && addressEquals(transfer.target, target);
				if (sameTarget && transfer.placement === placement) return;

				workspaceState.pendingOperation = pendingTransfer(
					pointerTransfer({
						sources: transfer.sources,
						target,
						placement,
					}),
				);
			}),
			Match.orElse(() => {}),
		);
	},
	updateTransferPlacement: (state: ProjectState, { placement }: { placement: Placement }) => {
		const workspaceState = state.workspace;
		Match.value(workspaceState.pendingOperation).pipe(
			Match.when({ _tag: "Transfer", value: { _tag: "Keyboard" } }, ({ value: transfer }) => {
				if (transfer.placement === placement) return;

				workspaceState.pendingOperation = pendingTransfer(
					keyboardTransfer({
						sources: transfer.sources,
						kind: transfer.kind,
						placement,
						restoreSelection: transfer.restoreSelection,
						restoreFocus: transfer.restoreFocus,
					}),
				);
			}),
			Match.orElse(() => {}),
		);
	},
	updateTransferKind: (state: ProjectState, { kind }: { kind: TransferKind }) => {
		const workspaceState = state.workspace;
		Match.value(workspaceState.pendingOperation).pipe(
			Match.when({ _tag: "Transfer", value: { _tag: "Keyboard" } }, ({ value: transfer }) => {
				if (transfer.kind === kind) return;

				workspaceState.pendingOperation = pendingTransfer(
					keyboardTransfer({
						sources: transfer.sources,
						kind,
						placement: transfer.placement,
						restoreSelection: transfer.restoreSelection,
						restoreFocus: transfer.restoreFocus,
					}),
				);
			}),
			Match.orElse(() => {}),
		);
	},
	clearPendingOperation: (state: ProjectState) => {
		state.workspace.pendingOperation = noPendingOperation;
	},
	/** Ends the pending operation and says why in its place. */
	refusePendingOperation: (state: ProjectState, { notice }: { notice: string }) => {
		state.workspace.pendingOperation = noPendingOperation;
		state.workspace.notice = notice;
	},
	clearNotice: (state: ProjectState) => {
		state.workspace.notice = null;
	},
	setDependencyCommitIds: (
		state: ProjectState,
		{ commitIds }: { commitIds: Array<string> | null },
	) => {
		const nextCommitIds = commitIds ?? [];
		if (
			state.workspace.dependencyCommitIds.length === nextCommitIds.length &&
			state.workspace.dependencyCommitIds.every(
				(commitId, index) => commitId === nextCommitIds[index],
			)
		)
			return;

		state.workspace.dependencyCommitIds = nextCommitIds;
	},
	checkAddress: (
		state: ProjectState,
		{ address, checked }: { address: CheckableAddress; checked: boolean },
	) => {
		const key = addressIdentityKey(address);
		if (checked) state.workspace.checkedAddresses[key] = address;
		else delete state.workspace.checkedAddresses[key];
	},
	checkAddresses: (
		state: ProjectState,
		{ addresses, checked }: { addresses: Array<CheckableAddress>; checked: boolean },
	) => {
		for (const address of addresses) {
			const key = addressIdentityKey(address);
			if (checked) state.workspace.checkedAddresses[key] = address;
			else delete state.workspace.checkedAddresses[key];
		}
	},
	clearCheckedAddresses: (state: ProjectState) => {
		state.workspace.checkedAddresses = {};
	},
	checkConflict: (
		state: ProjectState,
		{ conflict, checked }: { conflict: CheckedConflict; checked: boolean },
	) => {
		const key = conflictCheckKey(conflict);
		if (checked) state.workspace.checkedConflicts[key] = conflict;
		else delete state.workspace.checkedConflicts[key];
	},
	clearCheckedConflicts: (state: ProjectState) => {
		if (Object.keys(state.workspace.checkedConflicts).length === 0) return;
		state.workspace.checkedConflicts = {};
	},
	updateRewrittenCommitReferences: (
		state: ProjectState,
		{ replacedCommits }: { replacedCommits: Record<string, string> },
	) => {
		const workspaceState = state.workspace;

		if (workspaceState.diffCursor)
			workspaceState.diffCursor = remapDiffCursor(workspaceState.diffCursor, replacedCommits);

		for (const [key, conflict] of Object.entries(workspaceState.checkedConflicts)) {
			const newId = replacedCommits[conflict.commitId];
			if (newId === undefined) continue;
			delete workspaceState.checkedConflicts[key];
			const moved = { ...conflict, commitId: newId };
			workspaceState.checkedConflicts[conflictCheckKey(moved)] = moved;
		}

		for (const [key, address] of Object.entries(workspaceState.checkedAddresses)) {
			let newAddress: CheckableAddress | null = null;
			if (address._tag === "Commit") {
				const newId = replacedCommits[address.commitId];
				if (newId !== undefined)
					newAddress = commitAddress({ commitId: newId, changeId: address.changeId });
			} else if (address._tag === "File" && address.parent._tag === "Commit") {
				const newId = replacedCommits[address.parent.commitId];
				if (newId !== undefined) {
					newAddress = fileAddress({
						parent: commitFileParent({ commitId: newId, changeId: address.parent.changeId }),
						path: address.path,
					});
				}
			} else if (address._tag === "Hunk" && address.parent.parent._tag === "Commit") {
				const newId = replacedCommits[address.parent.parent.commitId];
				if (newId !== undefined) {
					newAddress = hunkAddress({
						...address,
						parent: {
							...address.parent,
							parent: commitFileParent({
								commitId: newId,
								changeId: address.parent.parent.changeId,
							}),
						},
					});
				}
			}
			if (!newAddress) continue;

			delete workspaceState.checkedAddresses[key];
			workspaceState.checkedAddresses[addressIdentityKey(newAddress)] = newAddress;
		}

		if (
			workspaceState.pendingOperation._tag === "InlineEdit" &&
			workspaceState.pendingOperation.address._tag === "Commit"
		) {
			const newId = replacedCommits[workspaceState.pendingOperation.address.commitId];
			if (newId !== undefined) {
				workspaceState.pendingOperation = pendingInlineEdit({
					address: commitAddress({
						commitId: newId,
						changeId: workspaceState.pendingOperation.address.changeId,
					}),
				});
			}
		}
	},
	toggleFiles: (state: ProjectState) => {
		state.filesVisible = !state.filesVisible;
	},
	setSelectedBranchTab: (
		state: ProjectState,
		{ branchName, tab }: { branchName: string; tab: BranchTab },
	) => {
		if (state.workspace.selectedBranchTabs[branchName] === tab) return;

		state.workspace.selectedBranchTabs[branchName] = tab;
	},

	toggleSegmentFolded: (state: ProjectState, { branchRef }: { branchRef: string }) => {
		if (state.workspace.foldedSegments[branchRef]) delete state.workspace.foldedSegments[branchRef];
		else state.workspace.foldedSegments[branchRef] = true;
	},
	/**
	 * Folds or unfolds several segments at once, for acting on a whole stack.
	 * Toggling each of them instead would invert a partly folded stack rather
	 * than bring it to one state.
	 */
	setSegmentsFolded: (
		state: ProjectState,
		{ branchRefs, folded }: { branchRefs: Array<string>; folded: boolean },
	) => {
		for (const branchRef of branchRefs) {
			if (folded) state.workspace.foldedSegments[branchRef] = true;
			else delete state.workspace.foldedSegments[branchRef];
		}
	},
	toggleBranchUnfolded: (state: ProjectState, { branchRef }: { branchRef: string }) => {
		branchesReducers.toggleUnfolded(state.branches, { branchRef });
	},
	setBranchesUnfolded: (
		state: ProjectState,
		{ branchRefs, unfolded }: { branchRefs: Array<string>; unfolded: boolean },
	) => {
		branchesReducers.setUnfolded(state.branches, { branchRefs, unfolded });
	},
	/** Pass `null` to close the filter, which also clears the query. */
	setUncommittedFilesFilter: (state: ProjectState, { filter }: { filter: string | null }) => {
		const workspaceState = state.workspace;
		if (workspaceState.uncommittedFilesFilter === filter) return;

		workspaceState.uncommittedFilesFilter = filter;
	},
	/** Pass `null` to close the filter, which also clears the query. */
	setFilesFilter: (state: ProjectState, { filter }: { filter: string | null }) => {
		const workspaceState = state.workspace;
		if (workspaceState.filesFilter === filter) return;

		workspaceState.filesFilter = filter;
	},
	toggleUncommittedFilesRecentFirst: (state: ProjectState) => {
		state.workspace.uncommittedFilesRecentFirst = !state.workspace.uncommittedFilesRecentFirst;
	},
	toggleUncommittedFilesDirectoryCollapsed: (state: ProjectState, { path }: { path: string }) => {
		const collapsed = state.workspace.uncommittedFilesCollapsedDirectories;
		if (collapsed[path]) delete collapsed[path];
		else collapsed[path] = true;
	},
	toggleFilesDirectoryCollapsed: (state: ProjectState, { path }: { path: string }) => {
		const collapsed = state.workspace.filesCollapsedDirectories;
		if (collapsed[path]) delete collapsed[path];
		else collapsed[path] = true;
	},
	setBranchSearch: (state: ProjectState, { search }: { search: string | null }) => {
		branchesReducers.setSearch(state.branches, { search });
	},
	toggleBranchFilter: (state: ProjectState, { filter }: { filter: BranchFilter }) => {
		branchesReducers.toggleFilter(state.branches, { filter });
	},
};

const selectCheckedAddresses = createSelector(
	(state: ProjectState) => state.workspace.checkedAddresses,
	(checkedAddresses): Array<CheckableAddress> => Object.values(checkedAddresses),
);

/** The checks belonging to `commitId`, so a different commit reads as none. */
const selectCheckedConflictsFor = createSelector(
	(state: ProjectState) => state.workspace.checkedConflicts,
	(_state: ProjectState, commitId: string) => commitId,
	(checkedConflicts, commitId): Array<CheckedConflict> =>
		Object.values(checkedConflicts).filter((conflict) => conflict.commitId === commitId),
);

const selectCheckedAddressKeys = createSelector(
	(state: ProjectState) => state.workspace.checkedAddresses,
	(checkedAddresses): Set<string> => new Set(Object.keys(checkedAddresses)),
);

type GroupedCheckedAddresses = {
	commits: Array<CommitAddress>;
	uncommittedFiles: Array<FileAddress>;
	filesByCommitId: Map<string, Array<FileAddress>>;
	filesByBranchRef: Map<string, Array<FileAddress>>;
	hunksByFileParent: Map<string, Array<HunkAddress>>;
};

const selectGroupedCheckedAddresses = createSelector(
	selectCheckedAddresses,
	(checkedAddresses): GroupedCheckedAddresses =>
		checkedAddresses.reduce<GroupedCheckedAddresses>(
			(acc, address) => {
				switch (address._tag) {
					case "Commit":
						acc.commits.push(address);
						break;
					case "File": {
						switch (address.parent._tag) {
							case "UncommittedChanges":
								acc.uncommittedFiles.push(address);
								break;
							case "Commit":
								acc.filesByCommitId.getOrInsert(address.parent.commitId, []).push(address);
								break;
							case "Branch":
								acc.filesByBranchRef
									.getOrInsert(decodeBytes(address.parent.branchRef), [])
									.push(address);
								break;
							default:
								address.parent satisfies never;
						}
						break;
					}
					case "Hunk": {
						const parentKey = addressIdentityKey(address.parent.parent);
						acc.hunksByFileParent.getOrInsert(parentKey, []).push(address);
						break;
					}
					default:
						address satisfies never;
				}

				return acc;
			},
			{
				commits: [],
				uncommittedFiles: [],
				filesByCommitId: new Map(),
				filesByBranchRef: new Map(),
				hunksByFileParent: new Map(),
			},
		),
);

const selectCheckedCommitIds = createSelector(
	selectGroupedCheckedAddresses,
	(checkedGroupedAddresses): Set<string> =>
		new Set(checkedGroupedAddresses.commits.map((address) => address.commitId)),
);

const selectCheckedUncommittedFilePaths = createSelector(
	selectGroupedCheckedAddresses,
	(checkedGroupedAddresses): Set<string> =>
		new Set(checkedGroupedAddresses.uncommittedFiles.map((address) => address.path)),
);

const selectCheckedAddressCount = createSelector(
	selectCheckedAddresses,
	(checkedAddresses) => checkedAddresses.length,
);

const selectDependencyCommitIds = createSelector(
	(state: ProjectState) => state.workspace.dependencyCommitIds,
	(commitIds): Set<string> => new Set(commitIds),
);

export const projectSelectors = {
	selectFilesVisible: (state: ProjectState) => state.filesVisible,
	/**
	 * The explicitly chosen tab, or `undefined` when none was picked — the
	 * caller supplies the default, since whether the Pull Request tab is worth
	 * opening on depends on forge data the store does not hold.
	 */
	selectBranchTab: (state: ProjectState, branchName: string): BranchTab | undefined =>
		state.workspace.selectedBranchTabs[branchName],

	selectUncommittedFilesFilter: (state: ProjectState) => state.workspace.uncommittedFilesFilter,
	selectUncommittedFilesRecentFirst: (state: ProjectState) =>
		state.workspace.uncommittedFilesRecentFirst,
	selectFilesFilter: (state: ProjectState) => state.workspace.filesFilter,
	selectUncommittedFilesCollapsedDirectories: (state: ProjectState) =>
		state.workspace.uncommittedFilesCollapsedDirectories,
	selectFilesCollapsedDirectories: (state: ProjectState) =>
		state.workspace.filesCollapsedDirectories,
	/** The diff cursor as stored; its siblings live in the URL. */
	selectDiffCursor: (state: ProjectState) => state.workspace.diffCursor,
	/** A primitive, so checking one conflict re-renders one card. */
	selectIsConflictChecked: (state: ProjectState, conflict: CheckedConflict): boolean =>
		conflictCheckKey(conflict) in state.workspace.checkedConflicts,
	selectCheckedConflicts: selectCheckedConflictsFor,
	selectPendingOperation: (state: ProjectState) => state.workspace.pendingOperation,
	selectNotice: (state: ProjectState) => state.workspace.notice,
	selectFoldedSegments: (state: ProjectState) => state.workspace.foldedSegments,
	selectSegmentFolded: (state: ProjectState, branchRef: string) =>
		state.workspace.foldedSegments[branchRef] === true,
	selectDependencyCommitIds,
	selectAddressChecked: (state: ProjectState, address: CheckableAddress) =>
		state.workspace.checkedAddresses[addressIdentityKey(address)] !== undefined,
	selectCheckedAddresses,
	selectCheckedAddressKeys,
	selectCheckedCommitIds,
	selectCheckedUncommittedFilePaths,
	selectCheckedAddressCount,
	// Checking has been defined in a flexible way to support heterogeneous items, however in the UI
	// we currently only allow a single context of checked items at a time, hence these selectors.
	selectCheckedAddressesContext: (state: ProjectState): CheckableAddress["_tag"] | null =>
		selectCheckedAddressCount(state) === 0
			? null
			: selectGroupedCheckedAddresses(state).commits.length > 0
				? "Commit"
				: selectGroupedCheckedAddresses(state).hunksByFileParent.size > 0
					? "Hunk"
					: "File",
	selectCanCheckCommits: (state: ProjectState) =>
		selectCheckedAddresses(state).length === selectGroupedCheckedAddresses(state).commits.length,
	selectCanCheckFiles: (state: ProjectState, fileParent: FileParent) => {
		switch (fileParent._tag) {
			case "UncommittedChanges":
				return (
					selectCheckedAddresses(state).length ===
					selectGroupedCheckedAddresses(state).uncommittedFiles.length
				);
			case "Commit":
				return (
					selectCheckedAddresses(state).length ===
					(selectGroupedCheckedAddresses(state).filesByCommitId.get(fileParent.commitId)?.length ??
						0)
				);
			// We currently don't support any operations on branch files.
			case "Branch":
				return false;
		}
	},
	selectCanCheckHunks: (state: ProjectState, fileParent: FileParent) => {
		// We currently don't support any operations on branch hunks.
		if (fileParent._tag === "Branch") return false;

		return (
			selectCheckedAddresses(state).length ===
			(selectGroupedCheckedAddresses(state).hunksByFileParent.get(addressIdentityKey(fileParent))
				?.length ?? 0)
		);
	},
	...getBranchesSelectors((state: ProjectState) => state.branches),
	...getUpstreamSelectors((state: ProjectState) => state.upstream),
};
