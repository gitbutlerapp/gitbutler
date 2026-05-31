import type { Commit } from "@gitbutler/but-sdk";

export const shortCommitId = (commitId: string): string => commitId.slice(0, 7);

export const commitTitle = (input: string): string => {
	const trimmed = input.trim();
	const _title = trimmed.split("\n")[0];
	const title = _title === "" ? undefined : _title;
	return title ?? "(no message)";
};

export const commitIsDiverged = (commit: Commit): boolean =>
	commit.state.type === "LocalAndRemote" && commit.state.subject !== commit.id;
