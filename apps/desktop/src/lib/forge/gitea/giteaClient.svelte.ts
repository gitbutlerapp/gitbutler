import { giteaApi } from "gitea-js";
import { InjectionToken } from "@gitbutler/core/context";
import type { GiteaProjectId } from "$lib/forge/gitea/types";

type GiteaInstance = ReturnType<typeof giteaApi>;

export const GITEA_CLIENT = new InjectionToken<GiteaClient>("GiteaClient");

export class GiteaClient {
	api: GiteaInstance | undefined;
	forkProjectId: GiteaProjectId | undefined;
	upstreamProjectId: GiteaProjectId | undefined;
	instanceUrl: string | undefined;

	constructor() {}

	set(
		instanceUrl: string,
		accessToken: string,
		forkProjectId: GiteaProjectId,
		upstreamProjectId: GiteaProjectId,
	) {
		this.instanceUrl = instanceUrl;
		this.forkProjectId = forkProjectId;
		this.upstreamProjectId = upstreamProjectId;
		this.api = giteaApi(instanceUrl, { token: accessToken });
	}
}

export function gitea(extra: unknown): {
	api: GiteaInstance;
	forkProjectId: GiteaProjectId;
	upstreamProjectId: GiteaProjectId;
} {
	if (!hasGitea(extra)) throw new Error("No Gitea client!");
	if (!extra.giteaClient.api) throw new Error("Failed to find Gitea client");
	if (!extra.giteaClient.forkProjectId) throw new Error("Failed to find fork project ID");
	if (!extra.giteaClient.upstreamProjectId) throw new Error("Failed to find upstream project ID");

	return {
		api: extra.giteaClient.api!,
		forkProjectId: extra.giteaClient.forkProjectId,
		upstreamProjectId: extra.giteaClient.upstreamProjectId,
	};
}

export function hasGitea(extra: unknown): extra is {
	giteaClient: GiteaClient;
} {
	return (
		!!extra &&
		typeof extra === "object" &&
		extra !== null &&
		"giteaClient" in extra &&
		extra.giteaClient instanceof GiteaClient
	);
}
