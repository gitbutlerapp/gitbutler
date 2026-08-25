import {
	useCommitDiscard,
	useCommitDiscardChanges,
	useCommitUncommit,
	useCommitUncommitChanges,
	useDiscardFileChanges,
	useDiscardWorktreeChanges,
} from "#ui/api/mutations.ts";
import { changesInWorktreeQueryOptions, headInfoQueryOptions } from "#ui/api/queries.ts";
import { getHeadInfoIndex } from "#ui/api/ref-info.ts";
import {
	addressIdentityKey,
	commitAddress,
	uncommittedChangesFileParent,
	type Address,
} from "#ui/addresses.ts";
import type { ButtonVariant } from "#ui/components/Button.tsx";
import { focusScope } from "#ui/focus-scopes.ts";
import {
	changesFileHotkeys,
	diffHotkeys,
	selectionOperationHotkeys,
	sidebarHotkeys,
} from "#ui/hotkeys.ts";
import { fileParentFromSources, resolveDiffSpecs } from "#ui/operations/diff-specs.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppSelector } from "#ui/store.ts";
import { setCursor, startAbsorb, startKeyboardTransfer } from "#ui/use-cursor.ts";
import type { AddressSpace } from "#ui/workspace/address-space.ts";
import { selectAfterDiscardedCommits } from "./WorkspaceLists/selectAfterDiscardedCommit.ts";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Match } from "effect";

type CheckedAction = {
	label: string;
	/** The chord that runs the same act from the list, shown so the toolbar teaches it. */
	hotkey?: string;
	variant?: ButtonVariant;
	enabled: boolean;
	run: () => void;
};

/**
 * The acts available on the checked set, as the row menus offer them but addressed to the set
 * rather than to a row. Checking is confined to one kind at a time, so which acts apply follows
 * from what is checked; an unactionable set gets none, and the bar shows its count alone.
 */
export const useCheckedActions = ({
	projectId,
	appliedAddressSpace,
}: {
	projectId: string;
	appliedAddressSpace: AddressSpace<Address>;
}): Array<CheckedAction> => {
	const queryClient = useQueryClient();
	const checkedAddresses = useAppSelector((state) =>
		projectSlice.selectors.selectCheckedAddresses(state, projectId),
	);
	const { data: headInfoIndex } = useQuery({
		...headInfoQueryOptions(projectId),
		select: getHeadInfoIndex,
	});

	// Hooks cannot be conditional, so a non-file set still has to name a parent; it gets no file
	// actions below, so which one it names is immaterial.
	const fileParent = fileParentFromSources(checkedAddresses);
	const { canDiscard, discard } = useDiscardFileChanges({
		projectId,
		fileParent: fileParent ?? uncommittedChangesFileParent,
	});
	const { isPending: isUncommitChangesPending, mutate: commitUncommitChanges } =
		useCommitUncommitChanges();
	const { isPending: isCommitDiscardPending, mutate: commitDiscard } = useCommitDiscard();
	const { isPending: isCommitUncommitPending, mutate: commitUncommit } = useCommitUncommit();
	const { isPending: isCommitDiscardChangesPending, mutate: commitDiscardChanges } =
		useCommitDiscardChanges();
	const { isPending: isDiscardWorktreePending, mutate: discardWorktreeChanges } =
		useDiscardWorktreeChanges();

	const cut = (sources: Array<Address>): CheckedAction => ({
		label: "Cut",
		hotkey: selectionOperationHotkeys.cut.hotkey,
		enabled: true,
		run: () => {
			startKeyboardTransfer({ sources, kind: "move" });
			focusScope("sidebar");
		},
	});

	const [first] = checkedAddresses;
	if (!first) return [];

	return Match.value(first).pipe(
		Match.withReturnType<Array<CheckedAction>>(),
		Match.tags({
			Commit: () => {
				if (!checkedAddresses.every((address) => address._tag === "Commit")) return [];
				const subjectCommitIds = checkedAddresses.map((address) => address.commitId);

				return [
					{
						label: "Copy",
						hotkey: sidebarHotkeys.copy.hotkey,
						enabled: true,
						run: () => {
							startKeyboardTransfer({
								sources: checkedAddresses,
								kind: "copy",
								placement: "above",
							});
							focusScope("sidebar");
						},
					},
					cut(checkedAddresses),
					{
						label: "Uncommit",
						hotkey: sidebarHotkeys.uncommitCommit.hotkey,
						enabled: !isCommitUncommitPending,
						run: () =>
							commitUncommit({ projectId, assignTo: null, subjectCommitIds, dryRun: false }),
					},
					{
						label: "Delete",
						hotkey: sidebarHotkeys.deleteCommit.hotkey,
						variant: "danger",
						enabled: !isCommitDiscardPending,
						run: () => {
							// The cursor has to leave the set before it goes; anchoring on the topmost
							// checked commit lands it beside the set rather than inside it.
							const anchor = checkedAddresses.reduce((topmost, address) => {
								const indexOf = (candidate: typeof address) =>
									appliedAddressSpace.indexByKey.get(addressIdentityKey(candidate)) ??
									Number.POSITIVE_INFINITY;
								return indexOf(address) < indexOf(topmost) ? address : topmost;
							});
							const selectionAfterDiscard = selectAfterDiscardedCommits({
								addressSpace: appliedAddressSpace,
								commit: anchor,
								discardedCommitIds: new Set(subjectCommitIds),
								headInfoIndex,
							});

							commitDiscard(
								{ projectId, subjectCommitIds, dryRun: false },
								{
									onSuccess: (response) => {
										let latest = selectionAfterDiscard;

										rewrite: if (latest?._tag === "Commit") {
											const newId = response.workspace.replacedCommits[latest.commitId];
											if (newId === undefined) break rewrite;

											latest = commitAddress({ commitId: newId, changeId: latest.changeId });
										}

										setCursor("applied", latest);
									},
								},
							);
						},
					},
				];
			},

			Hunk: (hunk) => {
				if (!checkedAddresses.every((address) => address._tag === "Hunk")) return [];
				// We currently don't support any operations on branch hunks.
				const parent = hunk.parent.parent;
				if (parent._tag === "Branch") return [];

				// Lines recovered from a binary file have no diff spec to name them by.
				const canUseHunks = checkedAddresses.every(
					(address) => !address.isResultOfBinaryToTextConversion,
				);
				// Taking lines away names them within their whole hunk, which is not the form the
				// parent alone implies for uncommitted lines.
				const resolveForRemoval = () =>
					resolveDiffSpecs({
						projectId,
						queryClient,
						sources: checkedAddresses,
						hunkAction: "discard",
					});

				const cutLines: CheckedAction = { ...cut(checkedAddresses), enabled: canUseHunks };
				const discardLines: CheckedAction = {
					label: "Discard",
					variant: "danger",
					enabled:
						canUseHunks &&
						(parent._tag === "Commit" ? !isCommitDiscardChangesPending : !isDiscardWorktreePending),
					run: () =>
						void resolveForRemoval().then((changes) => {
							if (!changes) return;

							if (parent._tag === "Commit") {
								commitDiscardChanges({
									projectId,
									commitId: parent.commitId,
									changes,
									dryRun: false,
								});
							} else {
								discardWorktreeChanges({ projectId, worktreeChanges: changes });
							}
						}),
				};

				if (parent._tag === "Commit") {
					return [
						cutLines,
						{
							label: "Uncommit",
							enabled: canUseHunks && !isUncommitChangesPending,
							run: () =>
								void resolveForRemoval().then((changes) => {
									if (!changes) return;

									commitUncommitChanges({
										projectId,
										commitId: parent.commitId,
										assignTo: null,
										changes,
										dryRun: false,
									});
								}),
						},
						discardLines,
					];
				}

				return [
					{
						label: "Absorb",
						hotkey: diffHotkeys.absorb.hotkey,
						enabled: canUseHunks,
						run: () => {
							// Checked hunks carry a path, but an absorb target names files by their bytes,
							// so their changes have to be looked up. One gone stale fails the whole set.
							const changesByPath = new Map(
								queryClient
									.getQueryData(changesInWorktreeQueryOptions(projectId).queryKey)
									?.changes.map((change) => [change.path, change]),
							);
							const hunks = checkedAddresses.flatMap((address) => {
								const change = changesByPath.get(address.parent.path);
								return change
									? [{ pathBytes: change.pathBytes, hunkHeader: address.hunkHeader }]
									: [];
							});
							if (hunks.length !== checkedAddresses.length) return;

							startAbsorb({
								sources: checkedAddresses,
								sourceTarget: { type: "hunks", subject: { hunks } },
							});
							focusScope("sidebar");
						},
					},
					cutLines,
					discardLines,
				];
			},

			File: () => {
				if (fileParent === null || !checkedAddresses.every((address) => address._tag === "File"))
					return [];

				const discardChanges: CheckedAction = {
					label: "Discard",
					hotkey: changesFileHotkeys.discard.hotkey,
					variant: "danger",
					enabled: canDiscard,
					run: () => void discard({ change: null, extendToCheckedFiles: true }),
				};

				return Match.value(fileParent).pipe(
					Match.withReturnType<Array<CheckedAction>>(),
					Match.tagsExhaustive({
						UncommittedChanges: () => [
							{
								label: "Absorb",
								hotkey: changesFileHotkeys.absorb.hotkey,
								enabled: true,
								run: () => {
									// Checked files carry only paths, so their changes have to be looked up.
									// One of them gone stale fails the whole set, as discarding one does.
									const paths = new Set(checkedAddresses.map((address) => address.path));
									const changes = queryClient
										.getQueryData(changesInWorktreeQueryOptions(projectId).queryKey)
										?.changes.filter((change) => paths.has(change.path));
									if (!changes || changes.length !== paths.size) return;

									startAbsorb({
										sources: checkedAddresses,
										sourceTarget: {
											type: "treeChanges",
											subject: { changes, assignedStackId: null },
										},
									});
									focusScope("sidebar");
								},
							},
							cut(checkedAddresses),
							discardChanges,
						],
						Commit: ({ commitId }) => [
							cut(checkedAddresses),
							{
								label: "Uncommit",
								hotkey: changesFileHotkeys.uncommit.hotkey,
								enabled: !isUncommitChangesPending,
								run: () => {
									void resolveDiffSpecs({
										projectId,
										queryClient,
										sources: checkedAddresses,
									}).then((changes) => {
										if (!changes) return;

										commitUncommitChanges({
											projectId,
											commitId,
											assignTo: null,
											changes,
											dryRun: false,
										});
									});
								},
							},
							discardChanges,
						],
						// We currently don't support any operations on branch files.
						Branch: () => [],
					}),
				);
			},
		}),
		Match.orElse(() => []),
	);
};
