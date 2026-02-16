import { Gitlab } from "@gitbeaker/rest";
import { InjectionToken } from "@gitbutler/core/context";
import type { GitLabProjectId } from "$lib/forge/gitlab/gitlab";

type GitlabInstance = InstanceType<typeof Gitlab<false>>;

export const GITLAB_CLIENT = new InjectionToken<GitLabClient>("GitLabClient");

export class GitLabClient {
	api: GitlabInstance | undefined;
	forkProjectId: string | undefined;
	upstreamProjectId: string | undefined;

	constructor() {}

	set(
		instanceUrl: string | undefined,
		accessToken: string,
		forkProjectId: GitLabProjectId,
		upstreamProjectId: GitLabProjectId,
	) {
		this.api = new Gitlab({
			host: instanceUrl,
			token: accessToken,
		});
		this.forkProjectId = forkProjectId;
		this.upstreamProjectId = upstreamProjectId;
	}
}

export function gitlab(extra: unknown): {
	api: GitlabInstance;
	forkProjectId: string;
	upstreamProjectId: string;
} {
	if (!hasGitLab(extra)) throw new Error("No GitLab client!");
	if (!extra.gitLabClient.api) throw new Error("Failed to find GitLab client");
	if (!extra.gitLabClient.forkProjectId) throw new Error("Failed to find fork project ID");
	if (!extra.gitLabClient.upstreamProjectId) throw new Error("Failed to find upstream project ID");

	// Equivalent to using the readable's `get` function
	return {
		api: extra.gitLabClient.api!,
		forkProjectId: extra.gitLabClient.forkProjectId,
		upstreamProjectId: extra.gitLabClient.upstreamProjectId,
	};
}

function hasGitLab(extra: unknown): extra is {
	gitLabClient: GitLabClient;
} {
	return (
		!!extra &&
		typeof extra === "object" &&
		extra !== null &&
		"gitLabClient" in extra &&
		extra.gitLabClient instanceof GitLabClient
	);
}
