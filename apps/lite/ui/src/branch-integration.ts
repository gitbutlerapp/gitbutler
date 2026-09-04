/**
 * @file The update-from-remote flow's data: the strategy-generated plan, the
 * dry-run preview of applying it, and the derivations the panel renders from
 * them. The apply mutation lives in `api/mutations.ts` with its peers.
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
		// The plan reads the live workspace and nothing invalidates this cache:
		// refetch whenever the flow opens rather than trusting a stale one. Not
		// on refocus, though — that would swap the plan under staged edits while
		// the dialog is open.
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

/** One scalpel gesture on the strategy's plan. */
export type PlanEdit =
	| { kind: "drop"; commitId: string }
	| { kind: "squashIntoParent"; commitId: string };

/** The commits a step stands for. */
export const stepCommitIds = (step: InteractiveIntegrationStep): Array<string> =>
	step.kind === "squash" ? step.commits : [step.commitId];

/**
 * Apply the scalpel's edits to a strategy's steps, which run in application
 * order — parent to child. Dropping removes the commit from its step, and
 * the step once nothing is left of it; squashing folds the commit's step
 * into the step before it. An edit whose commit no longer appears in any
 * step — the strategy changed under it — skips without effect, and merge
 * steps are left alone: their commit is a range marker, not content.
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

		switch (edit.kind) {
			case "drop": {
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
									// The step shrinks; what it says it is stays.
									message: step.kind === "squash" ? step.message : null,
								});
				break;
			}
			case "squashIntoParent": {
				const parent = out[index - 1];
				if (parent === undefined || parent.kind === "merge") break;
				out = out.toSpliced(index - 1, 2, {
					kind: "squash",
					commits: [...stepCommitIds(parent), ...stepCommitIds(step)],
					message: parent.kind === "squash" ? parent.message : null,
				});
				break;
			}
		}
	}
	return out;
};

/** A previewed commit and which side of the divergence it came from. */
export type PreviewRow = {
	commit: Commit;
	/**
	 * `shared` is everything below the divergence — history both sides already
	 * had — plus commits the plan created fresh, like a merge commit.
	 */
	origin: "incoming" | "local" | "shared";
	/**
	 * The previewed commit's pre-rewrite identities — the ids the plan's steps
	 * speak in, so a gesture on this row edits the right step. A squash-produced
	 * commit carries every constituent, so acting on the row acts on all of
	 * them; an unrewritten commit carries just itself.
	 */
	tracedIds: Array<string>;
};

/**
 * The previewed branch as rows, each commit traced back to the side it came
 * from. The preview rewrites commits, so a previewed id is followed through
 * `replacedCommits` before it is looked up among the incoming ones; a change
 * id carried over from an upstream commit is the fallback for rewrites the
 * map does not cover.
 *
 * `null` while the previewed workspace does not hold the branch — the preview
 * belongs to another plan, or the branch left the workspace under the flow.
 */
export const buildPreviewRows = ({
	workspace,
	branch,
	divergence,
}: {
	workspace: WorkspaceState;
	/** The branch's full ref name. */
	branch: string;
	divergence: IntegrationDivergenceDisplay;
}): Array<PreviewRow> | null => {
	// The branch's history can span several segments: when an incoming commit
	// survives the plan unrewritten, the remote-tracking ref still points at
	// it and names a segment of its own below the branch's. The outline wants
	// the whole lane, so segments below the branch are folded in until another
	// workspace branch takes over.
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

	// A squash maps several old ids onto one previewed commit, so the trace
	// collects them all rather than letting the last entry win.
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
		// A commit that folds both sides together counts as local work: it
		// keeps the user's content, and only pure incoming history should wear
		// the incoming treatment.
		const hasLocal =
			tracedIds.some((id) => localIds.has(id)) || localChangeIds.has(commit.changeId);
		const hasUpstream =
			tracedIds.some((id) => upstreamIds.has(id)) || upstreamChangeIds.has(commit.changeId);
		const origin: PreviewRow["origin"] =
			hasUpstream && !hasLocal ? "incoming" : hasLocal ? "local" : "shared";
		return { commit, origin, tracedIds };
	});
};
