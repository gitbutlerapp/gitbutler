/**
 * @file Known rewritten commit IDs and branch names are handled separately before reaching this
 * module.
 */

import { useQueries, useQuery } from "@tanstack/react-query";
import { branchParamRef } from "#ui/cursor-url.ts";
import {
	branchDiffQueryOptions,
	changesInWorktreeQueryOptions,
	commitDetailsWithLineStatsQueryOptions,
	headInfoQueryOptions,
	treeChangesDiffsQueryOptions,
} from "./api/queries.ts";
import { currentParams, remapSearchBranch } from "#ui/use-cursor.ts";
import { getHeadInfoIndex, type HeadInfoIndex } from "./api/ref-info.ts";
import { useEffect, useEffectEvent, useLayoutEffect, useRef } from "react";
import { useAppDispatch, useAppSelector } from "./store.ts";
import { projectSlice } from "./projects/state.ts";
import {
	fileAddress,
	addressIdentityKey,
	type FileParent,
	type Address,
	uncommittedChangesFileParent,
	weakFileParentIdentityKey,
} from "./addresses.ts";
import { decodeBytes, encodeBytes } from "./api/bytes.ts";
import { hunkContainsHunk } from "./hunk.ts";
import type { RefInfo, TreeChange } from "@gitbutler/but-sdk";
import { reviewedFilesQueryOptions, usePruneReviewedFiles } from "./reviewed-files.ts";

/**
 * Reconcile in-memory and persisted client state against current repository data, most notably
 * between Redux and React Query.
 *
 * This hook should be called very high up in the tree so that synchronous dispatches in layout
 * effects don't waste too much work. This hook remains subscribed to queries relevant to state that
 * may need reconciliation.
 */
export const useStateReconciler = (projectId: string): void => {
	const dispatch = useAppDispatch();

	// Commit cursors need no repair here: `change:` params re-resolve by change
	// id (encode-match), and a `commit:` param names an identity nothing survives.
	const reconcileSelectedBranch = useEffectEvent(
		(headInfo: RefInfo, headInfoIndex: HeadInfoIndex, prevHeadInfoIndex: HeadInfoIndex) => {
			const refName = branchParamRef(currentParams().applied);
			if (refName === null) return;

			const refBytes = encodeBytes(refName);
			if (headInfoIndex.isApplied(refBytes)) return;

			const prev = prevHeadInfoIndex.branchContextByRefBytes(refBytes);
			if (!prev) return;

			// We've no stable identifier for branches, so assume a rename retains its stack and segment
			// positions between snapshots.
			const sameSegmentBranch =
				headInfo.stacks[prev.stackIndex]?.segments[prev.segmentIndex]?.refName;
			if (
				!sameSegmentBranch ||
				prevHeadInfoIndex.branchContextByRefBytes(sameSegmentBranch.fullNameBytes)
			)
				return;

			remapSearchBranch(refName, decodeBytes(sameSegmentBranch.fullNameBytes));
		},
	);

	const checkedAddresses = useAppSelector((state) =>
		projectSlice.selectors.selectCheckedAddresses(state, projectId),
	);

	const checkedCommits = checkedAddresses.filter((address) => address._tag === "Commit");
	const reconcileCheckedCommits = useEffectEvent((headInfoIndex: HeadInfoIndex) => {
		const invalidated = checkedCommits.filter(
			(commit) => !headInfoIndex.commitContextByCommitId(commit.commitId),
		);

		if (invalidated.length > 0) {
			dispatch(
				projectSlice.actions.checkAddresses({ projectId, addresses: invalidated, checked: false }),
			);
		}
	});

	type FileScopedCheckedAddress = {
		address: Extract<Address, { _tag: "File" | "Hunk" }>;
		parent: FileParent;
		path: string;
	};
	const checkedFiles = checkedAddresses
		.values()
		.map((address): FileScopedCheckedAddress | null => {
			switch (address._tag) {
				case "File":
					return { address, parent: address.parent, path: address.path };
				case "Hunk":
					return { address, parent: address.parent.parent, path: address.parent.path };
				default:
					return null;
			}
		})
		.filter((x) => x != null)
		.toArray();

	const checkedUncommittedFiles = checkedFiles.filter(
		(file) => file.parent._tag === "UncommittedChanges",
	);
	const reviewedFilesContextId = weakFileParentIdentityKey(uncommittedChangesFileParent);
	const { data: reviewedUncommittedFiles } = useQuery(
		reviewedFilesQueryOptions(projectId, reviewedFilesContextId),
	);
	const { mutate: pruneReviewedFiles } = usePruneReviewedFiles();
	const reconcileCheckedUncommittedFiles = useEffectEvent(
		(worktreeChangesByPath: Map<string, TreeChange>) => {
			const invalidated = checkedUncommittedFiles
				.values()
				.map((file) => (worktreeChangesByPath.has(file.path) ? null : file.address))
				.filter((x) => x != null)
				.toArray();

			if (invalidated.length > 0) {
				dispatch(
					projectSlice.actions.checkAddresses({
						projectId,
						addresses: invalidated,
						checked: false,
					}),
				);
			}
		},
	);

	const checkedCommitFiles = checkedFiles
		.values()
		.map((file) => (file.parent._tag === "Commit" ? { ...file, parent: file.parent } : null))
		.filter((x) => x != null)
		.toArray();
	const reconcileCheckedCommitFiles = useEffectEvent(
		(
			headInfoIndex: HeadInfoIndex,
			checkedCommitFilesByCommitId: Map<string, Map<string, TreeChange>>,
		) => {
			const invalidated = checkedCommitFiles
				.values()
				.map((file) =>
					!headInfoIndex.commitContextByCommitId(file.parent.commitId) ||
					checkedCommitFilesByCommitId.get(file.parent.commitId)?.has(file.path) === false
						? file.address
						: null,
				)
				.filter((x) => x != null)
				.toArray();

			if (invalidated.length > 0) {
				dispatch(
					projectSlice.actions.checkAddresses({
						projectId,
						addresses: invalidated,
						checked: false,
					}),
				);
			}
		},
	);

	const checkedBranchFiles = checkedFiles
		.values()
		.map((file) => (file.parent._tag === "Branch" ? { ...file, parent: file.parent } : null))
		.filter((x) => x != null)
		.toArray();
	const reconcileCheckedBranchFiles = useEffectEvent(
		(headInfoIndex: HeadInfoIndex, checkedBranchFilesByBranchName: Map<string, Set<string>>) => {
			const invalidated = checkedBranchFiles
				.values()
				.map((file) =>
					!headInfoIndex.isApplied(file.parent.branchRef) ||
					checkedBranchFilesByBranchName.get(decodeBytes(file.parent.branchRef))?.has(file.path) ===
						false
						? file.address
						: null,
				)
				.filter((x) => x != null)
				.toArray();

			if (invalidated.length > 0) {
				dispatch(
					projectSlice.actions.checkAddresses({
						projectId,
						addresses: invalidated,
						checked: false,
					}),
				);
			}
		},
	);

	const { data: headInfo } = useQuery(headInfoQueryOptions(projectId));
	const headInfoIndex = headInfo ? getHeadInfoIndex(headInfo) : undefined;
	const prevHeadInfoIndexRef = useRef<HeadInfoIndex>(null);
	useLayoutEffect(() => {
		if (!headInfo || !headInfoIndex) return;

		const prevHeadInfoIndex = prevHeadInfoIndexRef.current;
		if (prevHeadInfoIndex) reconcileSelectedBranch(headInfo, headInfoIndex, prevHeadInfoIndex);

		reconcileCheckedCommits(headInfoIndex);

		prevHeadInfoIndexRef.current = headInfoIndex;
	}, [headInfo, headInfoIndex]);

	const { data: worktreeChangesByPath } = useQuery({
		...changesInWorktreeQueryOptions(projectId),
		select: (data) => new Map(data.changes.map((change) => [change.path, change])),
		enabled:
			checkedUncommittedFiles.length > 0 ||
			(reviewedUncommittedFiles && reviewedUncommittedFiles.size > 0),
	});

	useLayoutEffect(() => {
		if (!worktreeChangesByPath) return;

		reconcileCheckedUncommittedFiles(worktreeChangesByPath);
	}, [worktreeChangesByPath]);

	const pruneReviewedUncommittedFiles = useEffectEvent(
		(worktreeChangesByPath: Map<string, TreeChange>) => {
			if (!reviewedUncommittedFiles) return;

			const stalePaths = new Set(reviewedUncommittedFiles.keys()).difference(
				new Set(worktreeChangesByPath.keys()),
			);
			if (stalePaths.size === 0) return;

			pruneReviewedFiles({
				projectId,
				contextId: reviewedFilesContextId,
				paths: stalePaths,
			});
		},
	);
	useEffect(() => {
		if (!worktreeChangesByPath) return;

		pruneReviewedUncommittedFiles(worktreeChangesByPath);
	}, [reviewedUncommittedFiles, worktreeChangesByPath]);

	const checkedCommitFileCommitIds = new Set(
		checkedCommitFiles.map((file) => file.parent.commitId),
	);
	const checkedCommitFilesByCommitId = useQueries({
		queries: Array.from(checkedCommitFileCommitIds, (commitId) =>
			commitDetailsWithLineStatsQueryOptions({ projectId, commitId }),
		),
		combine: (results): Map<string, Map<string, TreeChange>> =>
			new Map(
				results
					.values()
					.map((result) =>
						result.data
							? ([
									result.data.commit.id,
									new Map(result.data.changes.map((change) => [change.path, change])),
								] as const)
							: null,
					)
					.filter((x) => x != null),
			),
	});
	useLayoutEffect(() => {
		if (!headInfoIndex) return;

		reconcileCheckedCommitFiles(headInfoIndex, checkedCommitFilesByCommitId);
	}, [headInfoIndex, checkedCommitFilesByCommitId]);

	const checkedBranchFileBranchNames = Array.from(
		new Set(checkedBranchFiles.map((file) => decodeBytes(file.parent.branchRef))),
	);
	const checkedBranchFilesByBranchName = useQueries({
		queries: checkedBranchFileBranchNames.map((branch) =>
			branchDiffQueryOptions({ projectId, branch }),
		),
		combine: (results) =>
			new Map(
				results
					.values()
					.map((result, idx) => {
						const key = checkedBranchFileBranchNames[idx];
						return key !== undefined && result.data
							? ([key, new Set(result.data.changes.map((change) => change.path))] as const)
							: null;
					})
					.filter((x) => x != null),
			),
	});
	useLayoutEffect(() => {
		if (!headInfoIndex) return;

		reconcileCheckedBranchFiles(headInfoIndex, checkedBranchFilesByBranchName);
	}, [headInfoIndex, checkedBranchFilesByBranchName]);

	const checkedHunks = checkedAddresses.filter((address) => address._tag === "Hunk");
	const checkedHunkFiles = Map.groupBy(checkedHunks, (hunk) =>
		addressIdentityKey(fileAddress(hunk.parent)),
	)
		.values()
		.map((hunks) => {
			const anyHunk = hunks[0];
			if (!anyHunk) return null;

			const { parent, path } = anyHunk.parent;

			const change =
				parent._tag === "UncommittedChanges"
					? worktreeChangesByPath?.get(path)
					: parent._tag === "Commit"
						? checkedCommitFilesByCommitId.get(parent.commitId)?.get(path)
						: undefined;
			return change ? { change, hunks } : null;
		})
		.filter((x) => x != null)
		.toArray();
	const { data: validCheckedHunkKeys = new Set<string>(), isFetching: checkingHunks } = useQuery({
		...treeChangesDiffsQueryOptions({
			projectId,
			changes: checkedHunkFiles.map(({ change }) => change),
		}),
		select: (treeChangeDiffs): Set<string> =>
			new Set(
				treeChangeDiffs
					.values()
					.map((patch, index) => {
						const file = checkedHunkFiles[index];
						return file && patch?.type === "Patch" ? { file, patch } : null;
					})
					.filter((x) => x != null)
					.flatMap(({ file, patch }) =>
						file.hunks
							.values()
							.filter(
								(hunk) =>
									hunk.isResultOfBinaryToTextConversion ===
										patch.subject.isResultOfBinaryToTextConversion &&
									patch.subject.hunks.some((current) => hunkContainsHunk(current, hunk.hunkHeader)),
							)
							.map(addressIdentityKey),
					),
			),
	});
	const reconcileCheckedHunks = useEffectEvent((validHunkKeys: Set<string>) => {
		const invalidated = checkedHunks.filter((hunk) => !validHunkKeys.has(addressIdentityKey(hunk)));

		if (invalidated.length > 0) {
			dispatch(
				projectSlice.actions.checkAddresses({ projectId, addresses: invalidated, checked: false }),
			);
		}
	});
	useLayoutEffect(() => {
		if (!checkingHunks) reconcileCheckedHunks(validCheckedHunkKeys);
	}, [validCheckedHunkKeys, checkingHunks]);
};
