import type { Commit, ForgeInfo } from "@gitbutler/but-sdk";

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
