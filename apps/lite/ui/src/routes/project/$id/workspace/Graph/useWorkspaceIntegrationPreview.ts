import { useWorkspaceIntegrateUpstream } from "#ui/api/mutations.ts";
import {
	changesInWorktreeQueryOptions,
	workspaceTargetCommitsQueryOptions,
	headInfoQueryOptions,
} from "#ui/api/queries.ts";
import { stackBottomRelativeTo } from "#ui/api/stack.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppSelector } from "#ui/store.ts";
import type { BottomUpdate, RefInfo, WorkspaceIntegrateUpstreamOutcome } from "@gitbutler/but-sdk";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { commitTitle } from "#ui/commit.ts";

type ConflictCommit = { id: string; title: string; files: Array<string> };

// IPC cannot cancel a running native call. Keep it registered until it settles,
// even if its query has been cancelled, so later revisions don't queue native work.
const activePreviews = new Map<string, Promise<WorkspaceIntegrateUpstreamOutcome>>();
const previewUpstreamConflicts = async (
	projectId: string,
	updates: Array<BottomUpdate>,
	signal: AbortSignal,
) => {
	signal.throwIfAborted();
	await new Promise<void>((resolve, reject) => {
		const onAbort = () => {
			clearTimeout(timer);
			reject(signal.reason);
		};
		const timer = setTimeout(() => {
			signal.removeEventListener("abort", onAbort);
			resolve();
		}, 200);
		signal.addEventListener("abort", onAbort, { once: true });
	});
	let active = activePreviews.get(projectId);
	while (active !== undefined) {
		await active.catch(() => undefined);
		signal.throwIfAborted();
		active = activePreviews.get(projectId);
	}
	signal.throwIfAborted();
	const request = window.lite.workspaceIntegrateUpstream({ projectId, updates, dryRun: true });
	activePreviews.set(projectId, request);
	try {
		return await request;
	} finally {
		activePreviews.delete(projectId);
	}
};

const conflictSummary = (result: WorkspaceIntegrateUpstreamOutcome, head: RefInfo | undefined) => {
	const current = new Map<string, { branch: string; commit: Omit<ConflictCommit, "files"> }>();
	for (const stack of head?.stacks ?? []) {
		for (const [index, segment] of stack.segments.entries()) {
			if (segment.commits.length === 0) continue;
			const named = segment.refName
				? segment
				: stack.segments.slice(index + 1).find((below) => below.refName !== null);
			const branchName = named?.refName?.displayName ?? "Unnamed branch";
			for (const commit of segment.commits) {
				current.set(result.workspaceState.replacedCommits[commit.id] ?? commit.id, {
					branch: branchName,
					commit: {
						id: commit.id,
						title: commitTitle(commit.message) ?? "(no message)",
					},
				});
			}
		}
	}
	const branches = new Map<string, Array<ConflictCommit>>();
	for (const stack of result.workspaceState.headInfo.stacks) {
		for (const [index, segment] of stack.segments.entries()) {
			if (segment.commits.length === 0) continue;
			const named = segment.refName
				? segment
				: stack.segments.slice(index + 1).find((below) => below.refName !== null);
			const branchName = named?.refName?.displayName ?? "Unnamed branch";
			for (const commit of segment.commits) {
				if (!commit.hasConflicts) continue;
				const original = current.get(commit.id);
				const branch = original?.branch ?? branchName;
				const commits = branches.get(branch) ?? [];
				commits.push({
					...(original?.commit ?? {
						id: commit.id,
						title: commitTitle(commit.message) ?? "(no message)",
					}),
					files: result.commitConflicts[commit.id] ?? [],
				});
				branches.set(branch, commits);
			}
		}
	}
	return {
		branches: Array.from(branches, ([name, commits]) => ({ name, commits })),
		files: result.worktreeConflicts,
		checkoutConflict: result.workspaceState.checkoutConflictOccurred,
	};
};

/** Share the preview and mutation between the target row and its docked copy. */
export const useWorkspaceIntegrationPreview = (projectId: string) => {
	const client = useQueryClient();
	const {
		data: headInfo,
		dataUpdatedAt: headRevision,
		isFetching: headFetching,
		isError: headFailed,
	} = useQuery(headInfoQueryOptions(projectId));
	const {
		dataUpdatedAt: targetRevision,
		isFetching: targetFetching,
		isError: targetFailed,
	} = useQuery(workspaceTargetCommitsQueryOptions(projectId));
	const {
		dataUpdatedAt: worktreeRevision,
		isFetching: worktreeFetching,
		isError: worktreeFailed,
	} = useQuery(changesInWorktreeQueryOptions(projectId));
	const noOperationPending = useAppSelector(
		(state) => projectSlice.selectors.selectPendingOperation(state, projectId)._tag === "None",
	);
	const { isPending, mutate: integrate } = useWorkspaceIntegrateUpstream();
	const updates = (headInfo?.stacks ?? [])
		.map(stackBottomRelativeTo)
		.filter((relativeTo) => relativeTo != null)
		.map((relativeTo): BottomUpdate => ({ kind: "rebase", selector: relativeTo }));
	const enabled = noOperationPending && headInfo?.target?.isCurrent === false && !isPending;
	const sourcesFetching = headFetching || targetFetching || worktreeFetching;
	const sourcesFailed = headFailed || targetFailed || worktreeFailed;
	const previewEnabled =
		enabled &&
		headRevision > 0 &&
		targetRevision > 0 &&
		worktreeRevision > 0 &&
		!sourcesFetching &&
		!sourcesFailed;
	const {
		data: conflicts,
		isFetching: previewFetching,
		isError: previewFailed,
	} = useQuery({
		queryKey: [
			projectId,
			"dryRun",
			"workspaceIntegrateUpstream",
			updates,
			headRevision,
			targetRevision,
			worktreeRevision,
			// Detach and abort waiting work when a source refresh or an operation starts.
			previewEnabled,
		],
		queryFn: ({ signal }) => previewUpstreamConflicts(projectId, updates, signal),
		enabled: previewEnabled,
		// The preview takes the repository lock. Recheck when source data changes, not on focus.
		staleTime: Infinity,
		refetchOnWindowFocus: false,
		retry: false,
		gcTime: 10_000,
		select: (result) => conflictSummary(result, headInfo),
	});
	const previewReady =
		previewEnabled && !previewFetching && !previewFailed && conflicts !== undefined;
	const rebase = () => {
		void client.cancelQueries({ queryKey: [projectId, "dryRun", "workspaceIntegrateUpstream"] });
		integrate({ projectId, updates, dryRun: false });
	};
	return {
		conflicts: previewReady ? conflicts : undefined,
		enabled,
		isPending,
		rebase,
	};
};
