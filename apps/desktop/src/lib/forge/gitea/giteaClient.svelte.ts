import { InjectionToken } from "@gitbutler/core/context";

export const GITEA_CLIENT = new InjectionToken<GiteaClient>("GiteaClient");

export class GiteaClient {
	host: string | undefined;
	token: string | undefined;
	owner: string | undefined;
	repo: string | undefined;

	constructor() {}

	set(host: string | undefined, token: string | undefined, owner: string | undefined, repo: string | undefined) {
		this.host = host;
		this.token = token;
		this.owner = owner;
		this.repo = repo;
	}

	get baseUrl(): string {
		if (!this.host) return "";
		// Ensure host ends with /api/v1
		const base = this.host.endsWith("/") ? this.host.slice(0, -1) : this.host;
		if (base.endsWith("/api/v1")) return base;
		return `${base}/api/v1`;
	}

	async fetch(path: string, init?: RequestInit): Promise<Response> {
		if (!this.token) throw new Error("No Gitea token!");
		const url = `${this.baseUrl}${path.startsWith("/") ? path : `/${path}`}`;
		const headers = new Headers(init?.headers);
		headers.set("Authorization", `token ${this.token}`);
		headers.set("Accept", "application/json");
		return await fetch(url, { ...init, headers });
	}
}

export function gitea(extra: unknown): GiteaClient {
	if (!hasGitea(extra)) throw new Error("No Gitea client!");
	return extra.giteaClient;
}

function hasGitea(extra: unknown): extra is {
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
