import {
	useCommitUncommitChanges,
	useDiscardFileChanges,
	useResolveWorktreeConflicts,
} from "#ui/api/mutations.ts";
import { changesInWorktreeQueryOptions, treeChangesDiffsQueryOptions } from "#ui/api/queries.ts";
import { weakFileParentIdentityKey, type Address, type FileParent } from "#ui/addresses.ts";
import { focusScope } from "#ui/focus-scopes.ts";
import { resolveDiffSpecs } from "#ui/operations/diff-specs.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useSetFilesReviewed } from "#ui/reviewed-files.ts";
import { useAppSelector, useAppStore } from "#ui/store.ts";
import { startAbsorb, startKeyboardTransfer } from "#ui/use-cursor.ts";
import type { TreeChange } from "@gitbutler/but-sdk";
import { useQueryClient } from "@tanstack/react-query";
import { prepareDiffFiles } from "./diff-view.ts";

/** A file address in the tagged form the checked set and the operation machinery carry. */
type FileSetAddress = Extract<Address, { _tag: "File" }>;

/**
 * The acts a list of files offers, addressed to a set rather than to one file: a row's
 * own file, the files below a directory row, or the checked set, whichever the caller
 * decides. Every act but reviewing names its subject by address and looks the changes
 * back up itself, so no caller has to hold a {@link TreeChange} to offer one; reviewing
 * names them outright, since what it records is a version of their diff.
 *
 * A subject with a stale member fails the whole act rather than acting on the rest —
 * the same all-or-nothing rule discarding the checked set has always followed.
 */
export const useFileSetActions = ({
	projectId,
	fileParent,
}: {
	projectId: string;
	fileParent: FileParent;
}) => {
	const queryClient = useQueryClient();
	const { canDiscard, discard } = useDiscardFileChanges({ projectId, fileParent });
	const { isPending: isUncommitPending, mutate: commitUncommitChanges } =
		useCommitUncommitChanges();
	const { isPending: isResolvePending, mutate: resolveWorktreeConflicts } =
		useResolveWorktreeConflicts();
	const { mutate: setFilesReviewed } = useSetFilesReviewed();
	// A linked worktree's files commit and amend into the workspace like any uncommitted
	// file, but have no absorb or discard yet: both act on the project's own checkout.
	const isLinkedWorktree =
		fileParent._tag === "UncommittedChanges" && fileParent.worktree !== undefined;

	return {
		// We currently don't support any operations on branch files.
		canCut: fileParent._tag !== "Branch",
		canAbsorb: fileParent._tag === "UncommittedChanges" && !isLinkedWorktree,
		canDiscard: canDiscard && !isLinkedWorktree,
		canUncommit: fileParent._tag === "Commit" && !isUncommitPending,
		canResolve: fileParent._tag === "UncommittedChanges" && !isResolvePending,

		cut: (addresses: Array<FileSetAddress>): void => {
			startKeyboardTransfer({ sources: addresses, kind: "move" });
			focusScope("sidebar");
		},

		absorb: (addresses: Array<FileSetAddress>): void => {
			// The subject carries paths, but an absorb target names files by their bytes, so
			// their changes have to be looked up. One gone stale fails the whole set.
			const paths = new Set(addresses.map((address) => address.path));
			const changes = queryClient
				.getQueryData(changesInWorktreeQueryOptions(projectId).queryKey)
				?.changes.filter((change) => paths.has(change.path));
			if (!changes || changes.length !== paths.size) return;

			startAbsorb({
				sources: addresses,
				sourceTarget: { type: "treeChanges", subject: { changes, assignedStackId: null } },
			});
			focusScope("sidebar");
		},

		discard: (addresses: Array<FileSetAddress>): Promise<void> => discard(addresses),

		uncommit: (addresses: Array<FileSetAddress>): void => {
			if (fileParent._tag !== "Commit") return;

			void resolveDiffSpecs({ projectId, queryClient, sources: addresses }).then((changes) => {
				if (!changes) return;

				commitUncommitChanges({
					projectId,
					commitId: fileParent.commitId,
					assignTo: null,
					changes,
					dryRun: false,
				});
			});
		},

		markResolved: (paths: Array<string>): void => {
			resolveWorktreeConflicts({ projectId, paths });
		},

		/**
		 * Reviewing records the version of the diff it saw, and only the patch carries
		 * one, so the diffs have to be in hand before the mark can be made. They are the
		 * ones the diff pane loads anyway, so they usually come from the cache.
		 */
		setReviewed: (changes: Array<TreeChange>, reviewed: boolean): void => {
			void queryClient
				.fetchQuery(
					treeChangesDiffsQueryOptions({
						projectId,
						changes,
						// As the diff pane asks for them: only uncommitted changes are read
						// from a checkout, so only they are diffed against a linked one.
						worktree: fileParent._tag === "UncommittedChanges" ? fileParent.worktree : undefined,
					}),
				)
				.then((treeChangeDiffs) => {
					setFilesReviewed({
						projectId,
						contextId: weakFileParentIdentityKey(fileParent),
						files: prepareDiffFiles({ fileParent, changes, treeChangeDiffs }).map(
							({ change, version }) => ({ path: change.path, version }),
						),
						reviewed,
					});
				});
		},
	};
};

/**
 * What a row's acts are addressed to. A row stands for its own files, but gives way to
 * the checked set when it is wholly part of one — a file row when it is checked, a
 * directory row when every file below it is. `count` reports the size so a label can
 * say what it is about to act on; `addresses` is read when the act runs, since the
 * checked set can move under a menu that is already open.
 */
export const useFileSetSubject = ({
	projectId,
	fileParent,
	promote,
	own,
	ownCount,
}: {
	projectId: string;
	fileParent: FileParent;
	/** Whether this row is wholly part of the checked set. */
	promote: boolean;
	own: () => Array<FileSetAddress>;
	ownCount: number;
}): { count: number; addresses: () => Array<FileSetAddress> } => {
	const store = useAppStore();
	// Gated on the set being wholly ours, so a mixed selection can't be mistaken for the
	// row's own subject.
	const checkedCount = useAppSelector((state) =>
		promote && projectSlice.selectors.selectCanCheckFiles(state, projectId, fileParent)
			? projectSlice.selectors.selectCheckedAddressCount(state, projectId)
			: 0,
	);
	const promoted = checkedCount > 0;

	return {
		count: promoted ? checkedCount : ownCount,
		addresses: () =>
			promoted
				? projectSlice.selectors
						.selectCheckedAddresses(store.getState(), projectId)
						.filter((address) => address._tag === "File")
				: own(),
	};
};
