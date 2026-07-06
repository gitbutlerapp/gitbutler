import { GITHUB_TEST_TOKEN, GITLAB_TEST_TOKEN } from "./env.ts";

/**
 * Minimal GitHub API helpers for verifying PR state.
 * Uses the REST API directly to avoid extra dependencies.
 */
export const github = {
	async listPullRequests(
		ownerRepo: string,
		opts?: { head?: string; state?: string },
	): Promise<GitHubPR[]> {
		const params = new URLSearchParams();
		if (opts?.head) params.set("head", opts.head);
		if (opts?.state) params.set("state", opts.state ?? "open");
		params.set("per_page", "100");

		const res = await fetch(
			`https://api.github.com/repos/${ownerRepo}/pulls?${params}`,
			{
				headers: {
					Authorization: `Bearer ${GITHUB_TEST_TOKEN}`,
					Accept: "application/vnd.github+json",
				},
			},
		);
		if (!res.ok) throw new Error(`GitHub API error: ${res.status} ${await res.text()}`);
		return (await res.json()) as GitHubPR[];
	},

	async closePullRequest(ownerRepo: string, prNumber: number): Promise<void> {
		const res = await fetch(
			`https://api.github.com/repos/${ownerRepo}/pulls/${prNumber}`,
			{
				method: "PATCH",
				headers: {
					Authorization: `Bearer ${GITHUB_TEST_TOKEN}`,
					Accept: "application/vnd.github+json",
				},
				body: JSON.stringify({ state: "closed" }),
			},
		);
		if (!res.ok) throw new Error(`GitHub API error closing PR: ${res.status}`);
	},

	async deleteRef(ownerRepo: string, ref: string): Promise<void> {
		const res = await fetch(
			`https://api.github.com/repos/${ownerRepo}/git/refs/heads/${ref}`,
			{
				method: "DELETE",
				headers: {
					Authorization: `Bearer ${GITHUB_TEST_TOKEN}`,
					Accept: "application/vnd.github+json",
				},
			},
		);
		// 422 = ref doesn't exist, that's fine.
		if (!res.ok && res.status !== 422) {
			throw new Error(`GitHub API error deleting ref: ${res.status}`);
		}
	},
};

/**
 * Minimal GitLab API helpers for verifying MR state.
 */
export const gitlab = {
	async listMergeRequests(
		projectPath: string,
		opts?: { sourceBranch?: string; state?: string },
	): Promise<GitLabMR[]> {
		const encoded = encodeURIComponent(projectPath);
		const params = new URLSearchParams();
		if (opts?.sourceBranch) params.set("source_branch", opts.sourceBranch);
		if (opts?.state) params.set("state", opts.state ?? "opened");
		params.set("per_page", "100");

		const res = await fetch(
			`https://gitlab.com/api/v4/projects/${encoded}/merge_requests?${params}`,
			{
				headers: { "PRIVATE-TOKEN": GITLAB_TEST_TOKEN },
			},
		);
		if (!res.ok) throw new Error(`GitLab API error: ${res.status} ${await res.text()}`);
		return (await res.json()) as GitLabMR[];
	},

	async closeMergeRequest(projectPath: string, mrIid: number): Promise<void> {
		const encoded = encodeURIComponent(projectPath);
		const res = await fetch(
			`https://gitlab.com/api/v4/projects/${encoded}/merge_requests/${mrIid}`,
			{
				method: "PUT",
				headers: {
					"PRIVATE-TOKEN": GITLAB_TEST_TOKEN,
					"Content-Type": "application/json",
				},
				body: JSON.stringify({ state_event: "close" }),
			},
		);
		if (!res.ok) throw new Error(`GitLab API error closing MR: ${res.status}`);
	},

	async deleteBranch(projectPath: string, branch: string): Promise<void> {
		const encoded = encodeURIComponent(projectPath);
		const res = await fetch(
			`https://gitlab.com/api/v4/projects/${encoded}/repository/branches/${encodeURIComponent(branch)}`,
			{
				method: "DELETE",
				headers: { "PRIVATE-TOKEN": GITLAB_TEST_TOKEN },
			},
		);
		if (!res.ok && res.status !== 404) {
			throw new Error(`GitLab API error deleting branch: ${res.status}`);
		}
	},
};

export interface GitHubPR {
	number: number;
	title: string;
	state: string;
	draft: boolean;
	head: { ref: string };
	base: { ref: string };
	body: string | null;
	auto_merge: { enabled: boolean } | null;
}

export interface GitLabMR {
	iid: number;
	title: string;
	state: string;
	draft: boolean;
	source_branch: string;
	target_branch: string;
	description: string | null;
}
