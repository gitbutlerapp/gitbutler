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
	createInitialGraphState,
	getGraphSelectors,
	graphReducers,
	type GraphState,
} from "./graph.ts";

/**
 * The workspace page's lists, in the order the details pane falls back
 * through them; the one named active drives the pane.
 */
export const activeLists = ["applied", "uncommitted"] as const;
export type ActiveList = (typeof activeLists)[number];

export type CheckableAddress = Extract<Address, { _tag: "Commit" | "File" | "Hunk" }>;

export type BranchTab = "diff" | "pr";

const allCommits: BranchCommitFilter = { _tag: "All" };

/**
 * Which of a branch's commits its diff shows: all of them, the ones not pushed yet, or an unbroken
 * run picked from its commit list, named by the ids of its oldest and newest commit.
 */
export type BranchCommitFilter =
	| { _tag: "All" }
	| { _tag: "Unpushed" }
	| { _tag: "Range"; oldest: string; newest: string };

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
	/**
	 * Remote legs shown, by the local branch's full ref. Expanded rather than
	 * folded, unlike `foldedSegments`: a leg starts collapsed.
	 */
	expandedIncoming: Record<string, true>;
	dependencyCommitIds: Array<string>;
	pendingOperation: PendingOperation;
	/**
	 * What became of the last operation that ended without one, stated where the operation's own
	 * controls stood. An operation that cannot run must not hold the workspace open waiting to be
	 * aimed, so it clears itself and leaves this behind to say why.
	 */
	notice: string | null;
	selectedBranchTabs: Record<string, BranchTab>;
	branchCommitFilters: Record<string, BranchCommitFilter>;
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
	expandedIncoming: {},
	dependencyCommitIds: [],
	pendingOperation: noPendingOperation,
	notice: null,
	selectedBranchTabs: {},
	branchCommitFilters: {},
	diffCursor: null,
	uncommittedFilesFilter: null,
	filesFilter: null,
	uncommittedFilesRecentFirst: false,
	uncommittedFilesCollapsedDirectories: {},
	filesCollapsedDirectories: {},
});

export type PageId = "workspace" | "branches";

export type ProjectState = {
	filesVisible: boolean;
	/** The uncommitted files card at the top of the graph, folded to its header row. */
	uncommittedFolded: boolean;
	branches: BranchesState;
	graph: GraphState;
	workspace: WorkspaceState;
};

export const createInitialProjectState = (): ProjectState => ({
	filesVisible: true,
	uncommittedFolded: false,
	branches: createInitialBranchesState(),
	graph: createInitialGraphState(),
	workspace: createInitialWorkspaceState(),
});

const setCheckedAddresses = (
	state: ProjectState,
	addresses: Array<CheckableAddress>,
	checked: boolean,
): void => {
	const stored = state.workspace.checkedAddresses;
	const files = new Set(
		addresses.filter((address) => address._tag === "File").map(addressIdentityKey),
	);
	if (files.size > 0) {
		for (const [key, address] of Object.entries(stored)) {
			if (address._tag === "Hunk" && files.has(addressIdentityKey(fileAddress(address.parent))))
				delete stored[key];
		}
	}
	for (const address of addresses) {
		const key = addressIdentityKey(address);
		if (!checked) delete stored[key];
		else if (
			address._tag !== "Hunk" ||
			(!files.has(addressIdentityKey(fileAddress(address.parent))) &&
				stored[addressIdentityKey(fileAddress(address.parent))] === undefined)
		)
			stored[key] ??= address;
	}
};

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
	toggleGraphIncoming: (state: ProjectState) => {
		graphReducers.toggleIncoming(state.graph);
	},
	toggleGraphHistory: (state: ProjectState) => {
		graphReducers.toggleHistory(state.graph);
	},
	showMoreGraphHistory: (state: ProjectState) => {
		graphReducers.showMoreHistory(state.graph);
	},
	showMoreGraphRun: (state: ProjectState, { runId }: { runId: string }) => {
		graphReducers.showMoreRun(state.graph, { runId });
	},
	foldGraphRun: (state: ProjectState, { runId }: { runId: string }) => {
		graphReducers.foldRun(state.graph, { runId });
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

		const oldRef = decodeBytes(oldBranch.branchRef);
		if (workspaceState.foldedSegments[oldRef]) {
			delete workspaceState.foldedSegments[oldRef];
			workspaceState.foldedSegments[decodeBytes(newBranch.branchRef)] = true;
		}

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
	) => setCheckedAddresses(state, [address], checked),
	checkAddresses: (
		state: ProjectState,
		{ addresses, checked }: { addresses: Array<CheckableAddress>; checked: boolean },
	) => setCheckedAddresses(state, addresses, checked),
	// Unlike raw removal during reconciliation, a user unchecking a covered line needs the
	// complete file's lines to retain everything outside that line, including folded hunks.
	checkLines: (
		state: ProjectState,
		{
			files,
			checked: linesToCheck,
			unchecked: linesToUncheck,
		}: {
			files: Array<{
				file: FileAddress;
				lines: Array<Extract<CheckableAddress, { _tag: "Hunk" }>>;
			}>;
			checked: Array<Extract<CheckableAddress, { _tag: "Hunk" }>>;
			unchecked: Array<Extract<CheckableAddress, { _tag: "Hunk" }>>;
		},
	) => {
		const stored = state.workspace.checkedAddresses;
		const keysToCheck = new Set(linesToCheck.map(addressIdentityKey));
		const keysToUncheck = new Set(linesToUncheck.map(addressIdentityKey));
		const affectedFileKeys = new Set<string>();
		const nextChecks = new Map<string, CheckableAddress>();

		for (const { file, lines: allLines } of files) {
			if (allLines.length === 0) continue;

			const wholeFileAddress = fileAddress(file);
			const fileKey = addressIdentityKey(wholeFileAddress);
			affectedFileKeys.add(fileKey);
			const isWholeFileChecked = stored[fileKey] !== undefined;
			const checkedLines = allLines.filter((line) => {
				const key = addressIdentityKey(line);
				if (keysToUncheck.has(key)) return false;
				if (keysToCheck.has(key)) return true;
				return isWholeFileChecked || stored[key] !== undefined;
			});

			if (checkedLines.length === allLines.length) {
				nextChecks.set(fileKey, wholeFileAddress);
				continue;
			}
			for (const line of checkedLines) nextChecks.set(addressIdentityKey(line), line);
		}

		// Keep unchanged entries untouched so repeating a check preserves state identity.
		for (const [key, address] of Object.entries(stored)) {
			if (address._tag === "Commit") continue;
			const fileKey = addressIdentityKey(
				address._tag === "File" ? address : fileAddress(address.parent),
			);
			if (affectedFileKeys.has(fileKey) && !nextChecks.has(key)) delete stored[key];
		}
		for (const [key, address] of nextChecks) stored[key] ??= address;
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
	toggleUncommittedFolded: (state: ProjectState) => {
		state.uncommittedFolded = !state.uncommittedFolded;
	},
	setSelectedBranchTab: (
		state: ProjectState,
		{ branchName, tab }: { branchName: string; tab: BranchTab },
	) => {
		if (state.workspace.selectedBranchTabs[branchName] === tab) return;

		state.workspace.selectedBranchTabs[branchName] = tab;
	},
	setBranchCommitFilter: (
		state: ProjectState,
		{ branchName, filter }: { branchName: string; filter: BranchCommitFilter },
	) => {
		if (filter._tag === "All") delete state.workspace.branchCommitFilters[branchName];
		else state.workspace.branchCommitFilters[branchName] = filter;
	},

	toggleSegmentFolded: (state: ProjectState, { branchRef }: { branchRef: string }) => {
		if (state.workspace.foldedSegments[branchRef]) delete state.workspace.foldedSegments[branchRef];
		else state.workspace.foldedSegments[branchRef] = true;
	},
	toggleIncomingExpanded: (state: ProjectState, { branchRef }: { branchRef: string }) => {
		if (state.workspace.expandedIncoming[branchRef])
			delete state.workspace.expandedIncoming[branchRef];
		else state.workspace.expandedIncoming[branchRef] = true;
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
	/** Total checked whole-file count pre-calculated for the selection label. */
	fileCount: number;
	/** Total checked line count, excluding checked whole-files, pre-calculated for the selection label. */
	lineCount: number;
	uncommittedFilesByWorktree: Map<string | undefined, Array<FileAddress>>;
	filesByCommitId: Map<string, Array<FileAddress>>;
	filesByBranchRef: Map<string, Array<FileAddress>>;
	hunksByFileParent: Map<string, Array<HunkAddress>>;
	hunkFileKeys: Set<string>;
};

const selectGroupedCheckedAddresses = createSelector(
	selectCheckedAddresses,
	selectCheckedAddressKeys,
	(checkedAddresses, checkedKeys): GroupedCheckedAddresses =>
		checkedAddresses.reduce<GroupedCheckedAddresses>(
			(acc, address) => {
				switch (address._tag) {
					case "Commit":
						acc.commits.push(address);
						break;
					case "File": {
						acc.fileCount++;
						switch (address.parent._tag) {
							case "UncommittedChanges":
								acc.uncommittedFilesByWorktree
									.getOrInsert(address.parent.worktree, [])
									.push(address);
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
						acc.hunkFileKeys.add(addressIdentityKey(fileAddress(address.parent)));
						if (!checkedKeys.has(addressIdentityKey(fileAddress(address.parent))))
							acc.lineCount += address.lineGroups.reduce((count, group) => count + group.lines, 0);
						break;
					}
					default:
						address satisfies never;
				}

				return acc;
			},
			{
				commits: [],
				fileCount: 0,
				lineCount: 0,
				uncommittedFilesByWorktree: new Map(),
				filesByCommitId: new Map(),
				filesByBranchRef: new Map(),
				hunksByFileParent: new Map(),
				hunkFileKeys: new Set(),
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
		new Set(
			checkedGroupedAddresses.uncommittedFilesByWorktree
				.values()
				.flatMap((files) => files.map((address) => address.path)),
		),
);

const selectCheckedAddressCount = createSelector(
	selectCheckedAddresses,
	(checkedAddresses) => checkedAddresses.length,
);

const selectDependencyCommitIds = createSelector(
	(state: ProjectState) => state.workspace.dependencyCommitIds,
	(commitIds): Set<string> => new Set(commitIds),
);

const selectCanCheckFilesOrHunks = (state: ProjectState, parent: FileParent): boolean => {
	if (parent._tag === "Branch") return false;

	const grouped = selectGroupedCheckedAddresses(state);
	const files =
		parent._tag === "UncommittedChanges"
			? grouped.uncommittedFilesByWorktree.get(parent.worktree)
			: grouped.filesByCommitId.get(parent.commitId);
	const hunks = grouped.hunksByFileParent.get(addressIdentityKey(parent));
	return selectCheckedAddressCount(state) === (files?.length ?? 0) + (hunks?.length ?? 0);
};

export const projectSelectors = {
	selectFilesVisible: (state: ProjectState) => state.filesVisible,
	selectUncommittedFolded: (state: ProjectState) => state.uncommittedFolded,
	/**
	 * The explicitly chosen tab, or `undefined` when none was picked — the
	 * caller supplies the default, since whether the Pull Request tab is worth
	 * opening on depends on forge data the store does not hold.
	 */
	selectBranchTab: (state: ProjectState, branchName: string): BranchTab | undefined =>
		state.workspace.selectedBranchTabs[branchName],
	selectBranchCommitFilter: (state: ProjectState, branchName: string): BranchCommitFilter =>
		state.workspace.branchCommitFilters[branchName] ?? allCommits,

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
	selectIncomingExpanded: (state: ProjectState, branchRef: string) =>
		state.workspace.expandedIncoming[branchRef] === true,
	selectDependencyCommitIds,
	selectAddressChecked: (state: ProjectState, address: CheckableAddress) =>
		state.workspace.checkedAddresses[addressIdentityKey(address)] !== undefined ||
		(address._tag === "Hunk" &&
			state.workspace.checkedAddresses[addressIdentityKey(fileAddress(address.parent))] !==
				undefined),
	selectCheckedAddresses,
	selectCheckedAddressKeys,
	selectCheckedHunkFileKeys: (state: ProjectState) =>
		selectGroupedCheckedAddresses(state).hunkFileKeys,
	selectCheckedCommitIds,
	selectCheckedUncommittedFilePaths,
	selectCheckedAddressCount,
	selectCheckedFileCount: (state: ProjectState) => selectGroupedCheckedAddresses(state).fileCount,
	selectCheckedLineCount: (state: ProjectState) => selectGroupedCheckedAddresses(state).lineCount,
	// Checking has been defined in a flexible way to support heterogeneous items, however in the UI
	// we currently only allow a single context of checked items at a time - with the exception of
	// files and hunks with the same parent - hence these selectors.
	selectCheckedAddressesContext: (
		state: ProjectState,
	): CheckableAddress["_tag"] | "FileAndHunk" | null => {
		if (selectCheckedAddressCount(state) === 0) return null;
		const grouped = selectGroupedCheckedAddresses(state);
		if (grouped.commits.length > 0) return "Commit";
		if (grouped.hunksByFileParent.size > 0) return grouped.fileCount > 0 ? "FileAndHunk" : "Hunk";
		return "File";
	},
	selectCanCheckCommits: (state: ProjectState) =>
		selectCheckedAddresses(state).length === selectGroupedCheckedAddresses(state).commits.length,
	selectCanCheckFilesOrHunks,
	selectCheckedSetIsFilesFromParent: (state: ProjectState, parent: FileParent) =>
		selectCanCheckFilesOrHunks(state, parent) &&
		selectGroupedCheckedAddresses(state).hunksByFileParent.size === 0,
	...getBranchesSelectors((state: ProjectState) => state.branches),
	...getGraphSelectors((state: ProjectState) => state.graph),
};
