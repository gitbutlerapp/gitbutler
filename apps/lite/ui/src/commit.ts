import { formatAbsoluteTime } from "@gitbutler/ui-react/time.ts";
import type { Commit, ForgeInfo } from "@gitbutler/but-sdk";
import { queryOptions, useMutation } from "@tanstack/react-query";
import * as idb from "idb-keyval";

const draftCommitMessageKey = (projectId: string): string => `commit_message_draft:v1:${projectId}`;

export const draftCommitMessageQueryOptions = (projectId: string) =>
	queryOptions({
		queryKey: [projectId, "commitMessageDraft"],
		queryFn: async () => (await idb.get<string>(draftCommitMessageKey(projectId))) ?? "",
	});

export const usePersistDraftCommitMessage = () =>
	useMutation({
		mutationFn: ({ projectId, message }: { projectId: string; message: string }) =>
			message === ""
				? idb.del(draftCommitMessageKey(projectId))
				: idb.set(draftCommitMessageKey(projectId), message),
		onSuccess: (_data, input, _res, ctx) =>
			ctx.client.setQueryData(
				draftCommitMessageQueryOptions(input.projectId).queryKey,
				input.message,
			),
	});

export const shortCommitId = (commitId: string): string => commitId.slice(0, 7);

/** Who and when, in the house absolute format; an empty name leaves the time. */
export const authorTooltip = (author: { name: string }, timestamp: number): string => {
	const when = formatAbsoluteTime(timestamp);
	return author.name === "" ? when : `${author.name} · ${when}`;
};

/**
 * The branch a published snapshot was taken of, if `message` is one: the hosted snapshot commit's
 * message is JSON describing it, which reads as noise where a title belongs.
 */
const snapshotBranch = (message: string): string | null => {
	if (!message.startsWith("{")) return null;
	try {
		const parsed: unknown = JSON.parse(message);
		const head = (parsed as { head?: unknown } | null)?.head;
		return typeof head === "string" ? head.replace(/^refs\/heads\//, "") : null;
	} catch {
		return null;
	}
};

export const commitTitle = (input: string): string | undefined => {
	const branch = snapshotBranch(input.trim());
	if (branch !== null) return `Uncommitted changes on ${branch}`;
	const trimmed = input.trim();
	const _title = trimmed.split("\n")[0];
	const title = _title === "" ? undefined : _title;
	return title;
};

export const commitBody = (input: string): string | undefined => {
	if (snapshotBranch(input.trim()) !== null) return undefined;
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
