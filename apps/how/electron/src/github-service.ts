import { dialog, shell, safeStorage } from "electron";
import fs from "node:fs/promises";
import path from "node:path";
import type {
	GithubAccount,
	GithubLoginResult,
	GithubRepositoriesResult,
	GithubRepositorySummary,
} from "./ipc.js";
import type { Logger } from "./logger.js";

type StoredCredential = {
	id: string;
	login: string;
	avatarUrl: string | null;
	encryptedToken: string;
};

type StoredCredentials = {
	activeCredentialId: string | null;
	credentials: Array<StoredCredential>;
};

type GithubDeviceVerification = {
	device_code: string;
	user_code: string;
	verification_uri: string;
	interval?: number;
};

type GithubAccessToken = {
	access_token?: string;
	error?: string;
	error_description?: string;
};

const defaultGithubClientId = "cd51880daa675d9e6452";

const emptyCredentials: StoredCredentials = {
	activeCredentialId: null,
	credentials: [],
};

export class GithubService {
	constructor(
		private readonly credentialsPath: string,
		private readonly logger: Logger,
	) {}

	async hasCredential(): Promise<boolean> {
		return (await this.#activeToken()) !== null;
	}

	async account(): Promise<GithubAccount | null> {
		const stored = await this.#readCredentials();
		const active = stored.credentials.find(
			(credential) => credential.id === stored.activeCredentialId,
		);
		if (!active) return null;
		return {
			login: active.login,
			avatarUrl: active.avatarUrl ?? null,
		};
	}

	async logout(): Promise<GithubAccount | null> {
		await fs.mkdir(path.dirname(this.credentialsPath), { recursive: true });
		await fs.writeFile(this.credentialsPath, `${JSON.stringify(emptyCredentials, null, 2)}\n`);
		return null;
	}

	async login(): Promise<GithubLoginResult> {
		if (process.env.HOW_E2E_GITHUB_LOGIN) {
			const avatarUrl = process.env.HOW_E2E_GITHUB_AVATAR_URL ?? null;
			await this.#storeToken("how-e2e-token", process.env.HOW_E2E_GITHUB_LOGIN, avatarUrl);
			return { type: "loggedIn", login: process.env.HOW_E2E_GITHUB_LOGIN, avatarUrl };
		}

		try {
			const verification = await this.#startDeviceFlow();
			await shell.openExternal(verification.verification_uri);
			const confirmation = await dialog.showMessageBox({
				type: "info",
				title: "Log in to GitHub",
				message: "Enter this code on GitHub",
				detail: verification.user_code,
				buttons: ["I logged in", "Cancel"],
				cancelId: 1,
			});
			if (confirmation.response === 1) throw new Error("GitHub login was cancelled.");
			this.logger.info("Started GitHub OAuth device flow", {
				verificationUri: verification.verification_uri,
				userCode: verification.user_code,
			});
			const token = await this.#pollDeviceFlow(verification);
			const account = await this.#fetchAccount(token);
			await this.#storeToken(token, account.login, account.avatarUrl);
			return { type: "loggedIn", login: account.login, avatarUrl: account.avatarUrl };
		} catch (error) {
			this.logger.error("GitHub login failed", error);
			return {
				type: "failed",
				message: "How could not log in to GitHub.",
			};
		}
	}

	async listRepositories(): Promise<GithubRepositoriesResult> {
		try {
			return { type: "repositories", repositories: await this.repositories() };
		} catch (error) {
			this.logger.error("Failed to list GitHub repositories", error);
			return {
				type: "failed",
				message: "How could not load GitHub projects.",
			};
		}
	}

	async repositories(): Promise<Array<GithubRepositorySummary>> {
		if (process.env.HOW_E2E_GITHUB_REPOSITORIES)
			return JSON.parse(process.env.HOW_E2E_GITHUB_REPOSITORIES) as Array<GithubRepositorySummary>;

		const token = await this.#requireToken();
		const repositories: Array<GithubRepositorySummary> = [];
		let page = 1;
		while (page <= 10) {
			const response = await this.#github<Array<Record<string, unknown>>>(
				`/user/repos?per_page=100&page=${page}&sort=updated&affiliation=owner,collaborator,organization_member`,
				token,
			);
			const publishable = response
				.filter((repository) => {
					const permissions = repository.permissions;
					return (
						typeof permissions === "object" &&
						permissions !== null &&
						((permissions as { push?: unknown }).push === true ||
							(permissions as { admin?: unknown }).admin === true)
					);
				})
				.map((repository) => ({
					id: String(repository.id),
					nameWithOwner: String(repository.full_name),
					cloneUrl: String(repository.clone_url),
					isPrivate: Boolean(repository.private),
				}));
			repositories.push(...publishable);
			if (response.length < 100) break;
			page += 1;
		}
		return repositories;
	}

	async createRepository(name: string): Promise<GithubRepositorySummary> {
		if (process.env.HOW_E2E_GITHUB_CREATE_REPO_URL) {
			const login = process.env.HOW_E2E_GITHUB_LOGIN ?? "how-e2e";
			return {
				id: `created-${name}`,
				nameWithOwner: `${login}/${name}`,
				cloneUrl: process.env.HOW_E2E_GITHUB_CREATE_REPO_URL,
				isPrivate: true,
			};
		}

		const token = await this.#requireToken();
		const repository = await this.#github<Record<string, unknown>>("/user/repos", token, {
			method: "POST",
			body: JSON.stringify({
				name,
				private: true,
				auto_init: false,
			}),
		});
		return {
			id: String(repository.id),
			nameWithOwner: String(repository.full_name),
			cloneUrl: String(repository.clone_url),
			isPrivate: Boolean(repository.private),
		};
	}

	async tokenForGit(): Promise<string | null> {
		return await this.#activeToken();
	}

	async #startDeviceFlow(): Promise<GithubDeviceVerification> {
		const response = await fetch("https://github.com/login/device/code", {
			method: "POST",
			headers: {
				accept: "application/json",
				"content-type": "application/json",
			},
			body: JSON.stringify({
				client_id: process.env.HOW_GITHUB_OAUTH_CLIENT_ID ?? defaultGithubClientId,
				scope: "repo",
			}),
		});
		if (!response.ok) throw new Error(`GitHub device flow failed: ${response.status}`);
		return (await response.json()) as GithubDeviceVerification;
	}

	async #pollDeviceFlow(verification: GithubDeviceVerification): Promise<string> {
		const startedAt = Date.now();
		const intervalMs = Math.max(verification.interval ?? 5, 1) * 1000;
		while (Date.now() - startedAt < 120_000) {
			await new Promise((resolve) => setTimeout(resolve, intervalMs));
			const response = await fetch("https://github.com/login/oauth/access_token", {
				method: "POST",
				headers: {
					accept: "application/json",
					"content-type": "application/json",
				},
				body: JSON.stringify({
					client_id: process.env.HOW_GITHUB_OAUTH_CLIENT_ID ?? defaultGithubClientId,
					device_code: verification.device_code,
					grant_type: "urn:ietf:params:oauth:grant-type:device_code",
				}),
			});
			if (!response.ok) throw new Error(`GitHub token request failed: ${response.status}`);
			const result = (await response.json()) as GithubAccessToken;
			if (result.access_token) return result.access_token;
			if (result.error === "authorization_pending") continue;
			if (result.error === "slow_down") continue;
			throw new Error(result.error_description ?? result.error ?? "GitHub login failed.");
		}
		throw new Error("GitHub login timed out.");
	}

	async #fetchAccount(token: string): Promise<GithubAccount> {
		const user = await this.#github<{ login: string; avatar_url?: string | null }>("/user", token);
		return {
			login: user.login,
			avatarUrl: typeof user.avatar_url === "string" ? user.avatar_url : null,
		};
	}

	async #github<T>(pathName: string, token: string, init: RequestInit = {}): Promise<T> {
		const response = await fetch(`https://api.github.com${pathName}`, {
			...init,
			headers: {
				accept: "application/vnd.github+json",
				authorization: `Bearer ${token}`,
				"content-type": "application/json",
				"X-GitHub-Api-Version": "2022-11-28",
				...init.headers,
			},
		});
		if (!response.ok) throw new Error(`GitHub request failed: ${response.status}`);
		return (await response.json()) as T;
	}

	async #requireToken(): Promise<string> {
		const token = await this.#activeToken();
		if (!token) throw new Error("How is not logged in to GitHub.");
		return token;
	}

	async #activeToken(): Promise<string | null> {
		const stored = await this.#readCredentials();
		const active = stored.credentials.find(
			(credential) => credential.id === stored.activeCredentialId,
		);
		if (!active) return null;
		return decryptToken(active.encryptedToken);
	}

	async #storeToken(token: string, login: string, avatarUrl: string | null): Promise<void> {
		const credential: StoredCredential = {
			id: "github",
			login,
			avatarUrl,
			encryptedToken: encryptToken(token),
		};
		await fs.mkdir(path.dirname(this.credentialsPath), { recursive: true });
		await fs.writeFile(
			this.credentialsPath,
			JSON.stringify(
				{
					activeCredentialId: credential.id,
					credentials: [credential],
				} satisfies StoredCredentials,
				null,
				2,
			),
		);
	}

	async #readCredentials(): Promise<StoredCredentials> {
		try {
			const parsed = JSON.parse(
				await fs.readFile(this.credentialsPath, "utf8"),
			) as StoredCredentials;
			if (!Array.isArray(parsed.credentials)) return emptyCredentials;
			return parsed;
		} catch {
			return emptyCredentials;
		}
	}
}

function encryptToken(token: string): string {
	if (safeStorage.isEncryptionAvailable())
		return `safe:${safeStorage.encryptString(token).toString("base64")}`;
	return `plain:${Buffer.from(token, "utf8").toString("base64")}`;
}

function decryptToken(stored: string): string | null {
	if (stored.startsWith("safe:")) {
		if (!safeStorage.isEncryptionAvailable()) return null;
		return safeStorage.decryptString(Buffer.from(stored.slice(5), "base64"));
	}
	if (stored.startsWith("plain:")) return Buffer.from(stored.slice(6), "base64").toString("utf8");
	return null;
}
