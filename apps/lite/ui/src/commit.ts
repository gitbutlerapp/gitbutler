import type { Commit, ForgeInfo } from "@gitbutler/but-sdk";
import type { CommitOperand } from "./operands.ts";
import type { HeadInfoIndex } from "./api/ref-info.ts";

export const shortCommitId = (commitId: string): string => commitId.slice(0, 7);

export const commitTitle = (input: string): string | undefined => {
	const trimmed = input.trim();
	const _title = trimmed.split("\n")[0];
	const title = _title === "" ? undefined : _title;
	return title;
};

export const commitBody = (input: string): string | undefined => {
	const trimmed = input.trim();
	const _body = trimmed.includes("\n") ? trimmed.slice(trimmed.indexOf("\n") + 1).trim() : "";
	const body = _body === "" ? undefined : _body;
	return body;
};

export const commitIsDiverged = (commit: Commit): boolean =>
	commit.state.type === "LocalAndRemote" && commit.state.subject !== commit.id;

type ForgeUrlFreshness = "fresh" | "stale";

/**
 * Builds a forge URL for commits present on the remote. May produce stale URLs for rewritten
 * commits that haven't been pushed yet.
 */
export const commitForgeUrl = (
	commit: Commit,
	forge: ForgeInfo,
): { url: string; freshness: ForgeUrlFreshness } | null => {
	if (commit.state.type === "LocalOnly") return null;

	const commitId = commit.state.type === "LocalAndRemote" ? commit.state.subject : commit.id;
	return {
		url: `${forge.baseUrl}${forge.commitUrlPath}${commitId}`,
		freshness: "subject" in commit.state && commit.state.subject !== commit.id ? "stale" : "fresh",
	};
};

/**
 * Map from old to new commit IDs. Helpful for resolving commits following new mutations. Not
 * oplog-aware.
 */
const rewrittenCommits = new Map<string, string>();

export const cacheRewrittenCommits = (rewrites: Record<string, string>): void => {
	for (const [k, v] of Object.entries(rewrites)) rewrittenCommits.set(k, v);
};

/**
 * Resolve a commit whose identity has potentially changed. Attempts to resolve by change ID, commit
 * ID, and rewritten commit ID. O(1)
 *
 * Each resolution option helps in different scenarios:
 *
 * 1. Commit IDs are globally unique (in practice), matching when a commit is unchanged.
 * 2. Rewritten commit IDs are also globally unique, resolving when an in-band mutation has
 *    occurred.
 * 3. Change ID headers are not necessarily globally unique and may not always be present, however
 *    they're valuable for many out-of-band mutations.
 *
 * Resolution may therefore for example fail in scenarios where a commit doesn't have a change ID
 * and a mutation is performed outside of GitButler, or where there is a change ID but it's not
 * preserved by other tooling.
 */
export const resolveCommit = (
	{ commitContextById, commitContextByChangeId }: HeadInfoIndex,
	selection: CommitOperand,
): CommitOperand | null => {
	const rid = rewrittenCommits.get(selection.commitId);

	// The order of the first two doesn't matter, but change ID should be last.
	const ctx =
		(rid !== undefined ? commitContextById(rid) : undefined) ??
		commitContextById(selection.commitId) ??
		commitContextByChangeId(selection.changeId);

	return ctx
		? {
				// oxlint-disable-next-line typescript/no-non-null-assertion -- [ref:stack-id-required]
				stackId: ctx.stack.id!,
				commitId: ctx.commit.id,
				changeId: ctx.commit.changeId,
			}
		: null;
};
