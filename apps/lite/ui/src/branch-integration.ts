/**
 * @file The update-from-remote flow's data: the plan, the dry-run preview,
 * and what the panel derives from them.
 */

import { decodeBytes } from "#ui/api/bytes.ts";
import type { PayloadFor } from "#electron/ipc.ts";
import type {
	Commit,
	IntegrationDivergenceDisplay,
	InteractiveIntegration,
	InteractiveIntegrationStep,
	WorkspaceState,
} from "@gitbutler/but-sdk";
import { queryOptions } from "@tanstack/react-query";

export const integrationPlanQueryOptions = ({
	projectId,
	branch,
	strategy,
}: PayloadFor<"getInitialBranchIntegration">) =>
	queryOptions({
		queryKey: [projectId, "branchIntegration", "plan", branch, strategy],
		queryFn: () => window.lite.getInitialBranchIntegration({ projectId, branch, strategy }),
		// Nothing invalidates this: refetch when the flow opens, but not on refocus,
		// which would swap the plan under staged edits.
		staleTime: 0,
		refetchOnWindowFocus: false,
		gcTime: 10_000,
	});

export const integrationPreviewQueryOptions = ({
	projectId,
	branch,
	integration,
}: Omit<PayloadFor<"applyBranchIntegration">, "dryRun" | "integration"> & {
	integration: InteractiveIntegration | undefined;
}) =>
	queryOptions({
		enabled: integration !== undefined,
		queryKey: [projectId, "branchIntegration", "preview", branch, integration],
		queryFn: () => {
			if (integration === undefined) return null;
			return window.lite.applyBranchIntegration({ projectId, branch, integration, dryRun: true });
		},
		staleTime: 0,
		// The dry run takes the worktree lock; a refocus must not re-run it.
		refetchOnWindowFocus: false,
		gcTime: 10_000,
	});

/** A commit left out of the plan, by its pre-rewrite id. */
export type PlanEdit = { commitId: string };

export const stepCommitIds = (step: InteractiveIntegrationStep): Array<string> =>
	step.kind === "squash" ? step.commits : [step.commitId];

/**
 * Dropping a commit removes it, and its step once empty. A commit no longer in
 * the plan is skipped; merge steps are left alone, their commit a range marker.
 */
export const applyPlanEdits = (
	steps: Array<InteractiveIntegrationStep>,
	edits: ReadonlyArray<PlanEdit>,
): Array<InteractiveIntegrationStep> => {
	let out = steps;
	for (const edit of edits) {
		const index = out.findIndex((step) => stepCommitIds(step).includes(edit.commitId));
		const step = out[index];
		if (step === undefined || step.kind === "merge") continue;

		const remaining = stepCommitIds(step).filter((id) => id !== edit.commitId);
		const head = remaining[0];
		out =
			head === undefined
				? out.toSpliced(index, 1)
				: remaining.length === 1
					? // A squash of one commit is that commit, kept as itself.
						out.toSpliced(index, 1, { kind: "pick", commitId: head })
					: out.toSpliced(index, 1, {
							kind: "squash",
							commits: remaining,
							// The shrunken step keeps its squash message.
							message: step.kind === "squash" ? step.message : null,
						});
	}
	return out;
};

/** A previewed commit and which side of the divergence it came from. */
export type PreviewRow = {
	commit: Commit;
	/** `shared`: history both sides had, plus commits the plan created, like a merge. */
	origin: "incoming" | "local" | "shared";
	/** The pre-rewrite ids the plan's steps use; a squash-produced commit carries every constituent. */
	tracedIds: Array<string>;
};

/**
 * The previewed branch as rows traced to their side: a previewed id is
 * followed through `replacedCommits`, then matched by change id. `null`
 * while the previewed workspace lacks the branch.
 */
export const buildPreviewRows = ({
	workspace,
	branch,
	divergence,
}: {
	workspace: WorkspaceState;
	branch: string;
	divergence: IntegrationDivergenceDisplay;
}): Array<PreviewRow> | null => {
	// A surviving incoming commit keeps its remote-tracking ref, a segment of its
	// own below the branch's; fold segments in until another branch takes over.
	let commits: Array<Commit> | undefined;
	stacks: for (const stack of workspace.headInfo.stacks) {
		for (const [index, segment] of stack.segments.entries()) {
			if (segment.refName === null || decodeBytes(segment.refName.fullNameBytes) !== branch)
				continue;
			commits = [...segment.commits];
			for (const below of stack.segments.slice(index + 1)) {
				const belowRef = below.refName === null ? null : decodeBytes(below.refName.fullNameBytes);
				if (belowRef !== null && belowRef.startsWith("refs/heads/")) break;
				commits.push(...below.commits);
			}
			break stacks;
		}
	}
	if (commits === undefined) return null;

	// A squash maps several old ids onto one commit; collect them all.
	const originsById = new Map<string, Array<string>>();
	for (const [oldId, newId] of Object.entries(workspace.replacedCommits)) {
		const origins = originsById.get(newId);
		if (origins === undefined) originsById.set(newId, [oldId]);
		else origins.push(oldId);
	}

	const upstreamIds = new Set(divergence.upstreamOnly.map((commit) => commit.id));
	const localIds = new Set(divergence.localOnly.map((commit) => commit.id));
	const upstreamChangeIds = new Set(
		divergence.upstreamOnly.flatMap((commit) =>
			commit.changeId !== null ? [commit.changeId] : [],
		),
	);
	const localChangeIds = new Set(
		divergence.localOnly.flatMap((commit) => (commit.changeId !== null ? [commit.changeId] : [])),
	);

	return commits.map((commit) => {
		const tracedIds = originsById.get(commit.id) ?? [commit.id];
		// A commit folding both sides counts as local; only pure incoming history is incoming.
		const hasLocal =
			tracedIds.some((id) => localIds.has(id)) || localChangeIds.has(commit.changeId);
		const hasUpstream =
			tracedIds.some((id) => upstreamIds.has(id)) || upstreamChangeIds.has(commit.changeId);
		const origin: PreviewRow["origin"] =
			hasUpstream && !hasLocal ? "incoming" : hasLocal ? "local" : "shared";
		return { commit, origin, tracedIds };
	});
};
