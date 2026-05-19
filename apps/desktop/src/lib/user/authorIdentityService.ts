import type { GitConfigService } from "$lib/config/gitConfigService"
import type { DefaultForgeFactory } from "$lib/forge/forgeFactory.svelte"
import { DEFAULT_HEADERS } from "$lib/forge/github/headers"
import type { GitHubClient } from "$lib/forge/github/githubClient"
import type { GitLabClient } from "$lib/forge/gitlab/gitlabClient.svelte"
import type { UserService } from "$lib/user/userService.svelte"
import { InjectionToken } from "@gitbutler/core/context"

export const AUTHOR_IDENTITY_SERVICE = new InjectionToken<AuthorIdentityService>(
	"AuthorIdentityService",
)

export type AuthorIdentityInput = {
	name?: string | null
	email?: string | null
	gravatarUrl?: string | null
}

export type ResolveAuthorIdentityOptions = {
	commitId?: string
}

export type ResolvedAuthorIdentity = {
	name: string
	email?: string
	avatarUrl?: string
}

type AuthorIdentityDependencies = {
	forgeFactory: DefaultForgeFactory
	gitHubClient: GitHubClient
	gitLabClient: GitLabClient
	gitConfigService: GitConfigService
	userService: UserService
}

type ProviderAuthorIdentity = {
	name?: string
	avatarUrl?: string
}

export function fallbackAuthorIdentity(
	author?: AuthorIdentityInput | null,
): ResolvedAuthorIdentity | undefined {
	if (!author) return undefined

	const email = valueOrUndefined(author.email)
	const name = valueOrUndefined(author.name) ?? email ?? "Unknown"

	return {
		name,
		email,
		avatarUrl: valueOrUndefined(author.gravatarUrl),
	}
}

export class AuthorIdentityService {
	private providerLookupCache = new Map<string, Promise<ProviderAuthorIdentity | undefined>>()
	private gitConfigEmailPromise?: Promise<string | undefined>

	constructor(private deps: AuthorIdentityDependencies) {}

	async resolve(
		author?: AuthorIdentityInput | null,
		options: ResolveAuthorIdentityOptions = {},
	): Promise<ResolvedAuthorIdentity | undefined> {
		const fallback = fallbackAuthorIdentity(author)
		if (!fallback) return undefined

		const currentUserIdentity = await this.resolveCurrentUserIdentity(author, fallback)
		if (currentUserIdentity) return currentUserIdentity

		const providerIdentity = await this.resolveProviderIdentity(author, options, fallback)
		if (!providerIdentity) return fallback

		return {
			...fallback,
			...providerIdentity,
			name: providerIdentity.name ?? fallback.name,
			avatarUrl: providerIdentity.avatarUrl ?? fallback.avatarUrl,
		}
	}

	async resolveMany(
		authors: ReadonlyArray<AuthorIdentityInput> | undefined,
	): Promise<ResolvedAuthorIdentity[]> {
		if (!authors?.length) return []

		return (
			await Promise.all(authors.map((author) => this.resolve(author).catch(() => fallbackAuthorIdentity(author))))
		).filter((author): author is ResolvedAuthorIdentity => !!author)
	}

	private async resolveCurrentUserIdentity(
		author: AuthorIdentityInput | null | undefined,
		fallback: ResolvedAuthorIdentity,
	): Promise<ResolvedAuthorIdentity | undefined> {
		const authorEmail = normalizeEmail(author?.email)
		if (!authorEmail) return undefined

		const currentUser = this.deps.userService.user
		if (!currentUser) return undefined

		const currentUserEmail = normalizeEmail(currentUser.email)
		const gitConfigEmail = normalizeEmail(await this.getGitConfigEmail())

		if (authorEmail !== currentUserEmail && authorEmail !== gitConfigEmail) {
			return undefined
		}

		return {
			name: currentUser.name ?? fallback.name,
			email: fallback.email,
			avatarUrl: currentUser.picture ?? fallback.avatarUrl,
		}
	}

	private async resolveProviderIdentity(
		author: AuthorIdentityInput | null | undefined,
		options: ResolveAuthorIdentityOptions,
		fallback: ResolvedAuthorIdentity,
	): Promise<ProviderAuthorIdentity | undefined> {
		const forgeName = this.deps.forgeFactory.current.name
		if (forgeName !== "github" && forgeName !== "gitlab") {
			return undefined
		}

		const cacheKey = this.providerCacheKey(forgeName, author, options)
		if (!cacheKey) return undefined

		const cached = this.providerLookupCache.get(cacheKey)
		if (cached) {
			return await cached
		}

		const lookup = this.lookupProviderIdentity(forgeName, author, options, fallback).catch((error) => {
			console.warn(this.providerLookupWarning(forgeName, error))
			return undefined
		})

		this.providerLookupCache.set(cacheKey, lookup)
		return await lookup
	}

	private async lookupProviderIdentity(
		forgeName: "github" | "gitlab",
		author: AuthorIdentityInput | null | undefined,
		options: ResolveAuthorIdentityOptions,
		fallback: ResolvedAuthorIdentity,
	): Promise<ProviderAuthorIdentity | undefined> {
		if (forgeName === "github") {
			return await this.lookupGitHubIdentity(author, options, fallback)
		}

		return await this.lookupGitLabIdentity(author, fallback)
	}

	private async lookupGitHubIdentity(
		author: AuthorIdentityInput | null | undefined,
		options: ResolveAuthorIdentityOptions,
		fallback: ResolvedAuthorIdentity,
	): Promise<ProviderAuthorIdentity | undefined> {
		const { owner, repo, octokit } = this.deps.gitHubClient

		if (options.commitId && owner && repo) {
			const commit = await octokit.rest.repos.getCommit({
				owner,
				repo,
				ref: options.commitId,
				headers: DEFAULT_HEADERS,
			})
			const providerAuthor = commit.data.author

			if (providerAuthor?.login) {
				return await this.lookupGitHubUser(providerAuthor.login, providerAuthor.avatar_url, fallback)
			}
		}

		const authorEmail = normalizeEmail(author?.email)
		if (!authorEmail) return undefined

		const search = await octokit.rest.search.users({
			q: `"${authorEmail}" in:email`,
			per_page: 1,
			headers: DEFAULT_HEADERS,
		})
		const providerAuthor = search.data.items.at(0)

		if (!providerAuthor?.login) return undefined
		return await this.lookupGitHubUser(providerAuthor.login, providerAuthor.avatar_url, fallback)
	}

	private async lookupGitHubUser(
		login: string,
		avatarUrl: string | undefined,
		fallback: ResolvedAuthorIdentity,
	): Promise<ProviderAuthorIdentity> {
		const user = await this.deps.gitHubClient.octokit.rest.users.getByUsername({
			username: login,
			headers: DEFAULT_HEADERS,
		})

		return {
			name: user.data.name ?? user.data.login ?? fallback.name,
			avatarUrl: user.data.avatar_url ?? avatarUrl ?? fallback.avatarUrl,
		}
	}

	private async lookupGitLabIdentity(
		author: AuthorIdentityInput | null | undefined,
		fallback: ResolvedAuthorIdentity,
	): Promise<ProviderAuthorIdentity | undefined> {
		const api = this.deps.gitLabClient.api
		const authorEmail = normalizeEmail(author?.email)
		if (!api || !authorEmail) return undefined

		const users = await api.Users.all({ search: authorEmail })
		const matchingUser = this.selectGitLabUser(users, authorEmail)
		if (!matchingUser) return undefined

		return {
			name: matchingUser.name ?? fallback.name,
			avatarUrl: valueOrUndefined(matchingUser.avatar_url) ?? fallback.avatarUrl,
		}
	}

	private selectGitLabUser(
		users: Array<{
			name?: string | null
			avatar_url?: string | null
			public_email?: unknown
			email?: unknown
		}>,
		email: string,
	) {
		return (
			users.find((user) => normalizeUnknownEmail(user.public_email) === email) ??
			users.find((user) => normalizeUnknownEmail(user.email) === email)
		)
	}

	private providerCacheKey(
		forgeName: "github" | "gitlab",
		author: AuthorIdentityInput | null | undefined,
		options: ResolveAuthorIdentityOptions,
	): string | undefined {
		const email = normalizeEmail(author?.email) ?? ""
		const commitId = valueOrUndefined(options.commitId) ?? ""

		if (forgeName === "github" && !email && !commitId) {
			return undefined
		}

		if (forgeName === "gitlab" && !email) {
			return undefined
		}

		if (forgeName === "github") {
			return [
				forgeName,
				valueOrUndefined(this.deps.gitHubClient.owner) ?? "",
				valueOrUndefined(this.deps.gitHubClient.repo) ?? "",
				email,
				commitId,
			].join("|")
		}

		return [
			forgeName,
			valueOrUndefined(this.deps.gitLabClient.upstreamProjectId) ?? "",
			email,
		].join("|")
	}

	private async getGitConfigEmail(): Promise<string | undefined> {
		if (!this.gitConfigEmailPromise) {
			this.gitConfigEmailPromise = this.deps.gitConfigService
				.get<string>("user.email")
				.then((email) => normalizeEmail(email))
				.catch(() => undefined)
		}

		return await this.gitConfigEmailPromise
	}

	private providerLookupWarning(forgeName: string, error: unknown): string {
		const message = error instanceof Error ? error.message : String(error)
		return `Failed to resolve ${forgeName} author identity: ${message}`
	}
}

export function createAuthorIdentityService({
	forgeFactory,
	gitHubClient,
	gitLabClient,
	gitConfigService,
	userService,
}: {
	forgeFactory: DefaultForgeFactory
	gitHubClient: GitHubClient
	gitLabClient: GitLabClient
	gitConfigService: GitConfigService
	userService: UserService
}) {
	return new AuthorIdentityService({
		forgeFactory,
		gitHubClient,
		gitLabClient,
		gitConfigService,
		userService,
	})
}

function normalizeEmail(email: string | null | undefined): string | undefined {
	return valueOrUndefined(email)?.trim().toLowerCase()
}

function normalizeUnknownEmail(email: unknown): string | undefined {
	return typeof email === "string" ? normalizeEmail(email) : undefined
}

function valueOrUndefined(value: string | null | undefined): string | undefined {
	return value && value.trim().length > 0 ? value : undefined
}
