/**
 * @file Known rewritten commit IDs and branch names are handled separately before reaching this
 * module.
 */

import { useQueries, useQuery } from "@tanstack/react-query";
import {
	branchDiffQueryOptions,
	changesInWorktreeQueryOptions,
	commitDetailsWithLineStatsQueryOptions,
	headInfoQueryOptions,
	treeChangeDiffsQueryOptions,
} from "./api/queries.ts";
import { useParams } from "@tanstack/react-router";
import { getHeadInfoIndex, type HeadInfoIndex } from "./api/ref-info.ts";
import { useEffect, useEffectEvent, useLayoutEffect, useRef } from "react";
import { useAppDispatch, useAppSelector } from "./store.ts";
import { projectSlice } from "./projects/state.ts";
import {
	branchOperand,
	commitOperand,
	fileOperand,
	operandIdentityKey,
	type FileParent,
	type Operand,
	uncommittedChangesFileParent,
	weakFileParentIdentityKey,
} from "./operands.ts";
import { decodeBytes } from "./api/bytes.ts";
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
export const useStateReconciler = (): void => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });

	const dispatch = useAppDispatch();

	const outlineSelection = useAppSelector((state) =>
		projectSlice.selectors.selectPrimaryOutlineSelection(state, projectId),
	);
	const reconcileSelectedCommit = useEffectEvent((headInfoIndex: HeadInfoIndex) => {
		if (outlineSelection?._tag !== "Commit") return;

		const curr = headInfoIndex.commitContextByCommitId(outlineSelection.commitId);
		if (curr) return;

		// Change IDs are not necessarily globally unique, but typically will be. In any case this
		// is a best-effort fallback.
		const commit = headInfoIndex.commitContextsByChangeId(outlineSelection.changeId)?.[0].commit;

		dispatch(
			projectSlice.actions.selectOutline({
				projectId,
				selection: commit
					? commitOperand({ commitId: commit.id, changeId: commit.changeId })
					: null,
			}),
		);
	});
	const reconcileSelectedBranch = useEffectEvent(
		(headInfo: RefInfo, headInfoIndex: HeadInfoIndex, prevHeadInfoIndex: HeadInfoIndex) => {
			if (outlineSelection?._tag !== "Branch") return;

			const curr = headInfoIndex.branchContextByRefBytes(outlineSelection.branchRef);
			if (curr) return;

			const prev = prevHeadInfoIndex.branchContextByRefBytes(outlineSelection.branchRef);
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

			dispatch(
				projectSlice.actions.selectOutline({
					projectId,
					selection: branchOperand({ branchRef: sameSegmentBranch.fullNameBytes }),
				}),
			);
		},
	);

	const checkedOperands = useAppSelector((state) =>
		projectSlice.selectors.selectCheckedOperands(state, projectId),
	);

	const checkedCommits = checkedOperands.filter((operand) => operand._tag === "Commit");
	const reconcileCheckedCommits = useEffectEvent((headInfoIndex: HeadInfoIndex) => {
		const invalidated = checkedCommits.filter(
			(commit) => !headInfoIndex.commitContextByCommitId(commit.commitId),
		);

		if (invalidated.length > 0) {
			dispatch(
				projectSlice.actions.checkOperands({ projectId, operands: invalidated, checked: false }),
			);
		}
	});

	type FileScopedCheckedOperand = {
		operand: Extract<Operand, { _tag: "File" | "Hunk" }>;
		parent: FileParent;
		path: string;
	};
	const checkedFiles = checkedOperands.flatMap<FileScopedCheckedOperand>((operand) => {
		switch (operand._tag) {
			case "File":
				return [{ operand, parent: operand.parent, path: operand.path }];
			case "Hunk":
				return [{ operand, parent: operand.parent.parent, path: operand.parent.path }];
			default:
				return [];
		}
	});

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
			const invalidated = checkedUncommittedFiles.flatMap((file) =>
				worktreeChangesByPath.has(file.path) ? [] : file.operand,
			);

			if (invalidated.length > 0) {
				dispatch(
					projectSlice.actions.checkOperands({ projectId, operands: invalidated, checked: false }),
				);
			}
		},
	);

	const checkedCommitFiles = checkedFiles.flatMap((file) =>
		file.parent._tag === "Commit" ? { ...file, parent: file.parent } : [],
	);
	const reconcileCheckedCommitFiles = useEffectEvent(
		(
			headInfoIndex: HeadInfoIndex,
			checkedCommitFilesByCommitId: Map<string, Map<string, TreeChange>>,
		) => {
			const invalidated = checkedCommitFiles.flatMap((file) =>
				!headInfoIndex.commitContextByCommitId(file.parent.commitId) ||
				checkedCommitFilesByCommitId.get(file.parent.commitId)?.has(file.path) === false
					? file.operand
					: [],
			);

			if (invalidated.length > 0) {
				dispatch(
					projectSlice.actions.checkOperands({ projectId, operands: invalidated, checked: false }),
				);
			}
		},
	);

	const checkedBranchFiles = checkedFiles.flatMap((file) =>
		file.parent._tag === "Branch" ? { ...file, parent: file.parent } : [],
	);
	const reconcileCheckedBranchFiles = useEffectEvent(
		(headInfoIndex: HeadInfoIndex, checkedBranchFilesByBranchName: Map<string, Set<string>>) => {
			const invalidated = checkedBranchFiles.flatMap((file) =>
				!headInfoIndex.branchContextByRefBytes(file.parent.branchRef) ||
				checkedBranchFilesByBranchName.get(decodeBytes(file.parent.branchRef))?.has(file.path) ===
					false
					? file.operand
					: [],
			);

			if (invalidated.length > 0) {
				dispatch(
					projectSlice.actions.checkOperands({ projectId, operands: invalidated, checked: false }),
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

		reconcileSelectedCommit(headInfoIndex);
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
				results.flatMap((result) =>
					result.data
						? [
								[
									result.data.commit.id,
									new Map(result.data.changes.map((change) => [change.path, change])),
								] as const,
							]
						: [],
				),
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
				results.flatMap((result, idx) => {
					const key = checkedBranchFileBranchNames[idx];
					return key !== undefined && result.data
						? [[key, new Set(result.data.changes.map((change) => change.path))]]
						: [];
				}),
			),
	});
	useLayoutEffect(() => {
		if (!headInfoIndex) return;

		reconcileCheckedBranchFiles(headInfoIndex, checkedBranchFilesByBranchName);
	}, [headInfoIndex, checkedBranchFilesByBranchName]);

	const checkedHunks = checkedOperands.filter((operand) => operand._tag === "Hunk");
	const checkedHunkFiles = Map.groupBy(checkedHunks, (hunk) =>
		operandIdentityKey(fileOperand(hunk.parent)),
	)
		.values()
		.flatMap((hunks) => {
			const anyHunk = hunks[0];
			if (!anyHunk) return [];
			const { parent, path } = anyHunk.parent;

			const change =
				parent._tag === "UncommittedChanges"
					? worktreeChangesByPath?.get(path)
					: parent._tag === "Commit"
						? checkedCommitFilesByCommitId.get(parent.commitId)?.get(path)
						: undefined;
			return change ? [{ change, hunks }] : [];
		})
		.toArray();
	const validCheckedHunkKeys = useQueries({
		queries: checkedHunkFiles.map(({ change }) =>
			treeChangeDiffsQueryOptions({ projectId, change }),
		),
		combine: (results): Set<string> =>
			new Set(
				results.flatMap(({ data: patch }, index) => {
					const file = checkedHunkFiles[index];
					if (!file || patch?.type !== "Patch") return [];

					return file.hunks.flatMap((hunk) =>
						hunk.isResultOfBinaryToTextConversion ===
							patch.subject.isResultOfBinaryToTextConversion &&
						patch.subject.hunks.some((current) => hunkContainsHunk(current, hunk.hunkHeader))
							? operandIdentityKey(hunk)
							: [],
					);
				}),
			),
	});
	const reconcileCheckedHunks = useEffectEvent((validHunkKeys: Set<string>) => {
		const invalidated = checkedHunks.filter((hunk) => !validHunkKeys.has(operandIdentityKey(hunk)));

		if (invalidated.length > 0) {
			dispatch(
				projectSlice.actions.checkOperands({ projectId, operands: invalidated, checked: false }),
			);
		}
	});
	useLayoutEffect(() => {
		reconcileCheckedHunks(validCheckedHunkKeys);
	}, [validCheckedHunkKeys]);
};
