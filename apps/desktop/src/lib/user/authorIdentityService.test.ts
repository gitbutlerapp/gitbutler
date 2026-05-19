import { AuthorIdentityService } from "$lib/user/authorIdentityService"
import { describe, expect, test, vi } from "vitest"
import type { DefaultForgeFactory } from "$lib/forge/forgeFactory.svelte"
import type { GitHubClient } from "$lib/forge/github/githubClient"
import type { GitLabClient } from "$lib/forge/gitlab/gitlabClient.svelte"
import type { GitConfigService } from "$lib/config/gitConfigService"
import type { UserService } from "$lib/user/userService.svelte"

type MockGitHubClientInput = {
	owner?: string
	repo?: string
	octokit?: unknown
}

type MockGitLabClientInput = {
	upstreamProjectId?: string
	api?: unknown
}

describe.concurrent("AuthorIdentityService", () => {
	test("prefers the signed-in user picture and name on non-provider forges when the commit email matches", async () => {
		const searchUsers = vi.fn()
		const getByUsername = vi.fn()
		const service = createService({
			forgeName: "default",
			gitHubClient: {
				owner: "gitbutler",
				repo: "gitbutler",
				octokit: {
					rest: {
						users: {
							getAuthenticated: vi.fn(),
							listEmailsForAuthenticatedUser: vi.fn(),
							getByUsername,
						},
						repos: { getCommit: vi.fn() },
						search: { users: searchUsers },
					},
				},
			},
			user: {
				email: "signed-in@example.com",
				name: "Signed In User",
				picture: "https://example.com/me.png",
			},
			gitConfigEmail: "signed-in@example.com",
		})

		const result = await service.resolve({
			name: "Commit Signature",
			email: "signed-in@example.com",
			gravatarUrl: "https://gravatar.example/avatar",
		})

		expect(result).toEqual({
			name: "Signed In User",
			email: "signed-in@example.com",
			avatarUrl: "https://example.com/me.png",
		})
		expect(searchUsers).not.toHaveBeenCalled()
		expect(getByUsername).not.toHaveBeenCalled()
	})

	test("prefers the authenticated GitHub account name and avatar for matching commit emails", async () => {
		const getAuthenticated = vi.fn().mockResolvedValue({
			data: {
				login: "yeusepe",
				name: "Yeusepe GH",
				email: null,
				avatar_url: "https://example.com/github-me.png",
			},
		})
		const listEmailsForAuthenticatedUser = vi.fn().mockResolvedValue({
			data: [{ email: "signed-in@example.com" }],
		})
		const getCommit = vi.fn()
		const searchUsers = vi.fn()
		const getByUsername = vi.fn()
		const service = createService({
			forgeName: "github",
			gitHubClient: {
				owner: "gitbutler",
				repo: "gitbutler",
				octokit: {
					rest: {
						repos: { getCommit },
						search: { users: searchUsers },
						users: {
							getAuthenticated,
							listEmailsForAuthenticatedUser,
							getByUsername,
						},
					},
				},
			},
			user: {
				email: "signed-in@example.com",
				name: "GitButler User",
				picture: "https://example.com/gitbutler-me.png",
			},
			gitConfigEmail: "signed-in@example.com",
		})

		const result = await service.resolve({
			name: "Commit Signature",
			email: "signed-in@example.com",
			gravatarUrl: "https://gravatar.example/avatar",
		})

		expect(result).toEqual({
			name: "Yeusepe GH",
			email: "signed-in@example.com",
			avatarUrl: "https://example.com/github-me.png",
		})
		expect(getAuthenticated).toHaveBeenCalledOnce()
		expect(listEmailsForAuthenticatedUser).toHaveBeenCalledOnce()
		expect(getCommit).not.toHaveBeenCalled()
		expect(searchUsers).not.toHaveBeenCalled()
		expect(getByUsername).not.toHaveBeenCalled()
	})

	test("uses the GitHub commit author account to enrich another author", async () => {
		const getCommit = vi.fn().mockResolvedValue({
			data: {
				author: {
					login: "octocat",
					avatar_url: "https://example.com/octocat-summary.png",
				},
			},
		})
		const searchUsers = vi.fn()
		const getByUsername = vi.fn().mockResolvedValue({
			data: {
				name: "The Octocat",
				login: "octocat",
				avatar_url: "https://example.com/octocat-profile.png",
			},
		})
		const service = createService({
			forgeName: "github",
			gitHubClient: {
				owner: "gitbutler",
				repo: "gitbutler",
				octokit: {
					rest: {
						repos: { getCommit },
						search: { users: searchUsers },
						users: {
							getAuthenticated: vi.fn().mockResolvedValue({
								data: {
									login: "someone-else",
									name: "Someone Else",
									email: "someone-else@example.com",
									avatar_url: "https://example.com/someone-else.png",
								},
							}),
							listEmailsForAuthenticatedUser: vi.fn().mockResolvedValue({
								data: [{ email: "someone-else@example.com" }],
							}),
							getByUsername,
						},
					},
				},
			},
		})

		const result = await service.resolve(
			{
				name: "Raw Commit Name",
				email: "octocat@example.com",
				gravatarUrl: "https://gravatar.example/octocat",
			},
			{ commitId: "deadbeef" },
		)

		expect(result).toEqual({
			name: "The Octocat",
			email: "octocat@example.com",
			avatarUrl: "https://example.com/octocat-profile.png",
		})
		expect(getCommit).toHaveBeenCalledOnce()
		expect(searchUsers).not.toHaveBeenCalled()
		expect(getByUsername).toHaveBeenCalledWith({
			username: "octocat",
			headers: {
				"X-GitHub-Api-Version": "2022-11-28",
				"If-None-Match": "",
			},
		})
	})

	test("falls back from a missing GitHub commit to email search", async () => {
		const getCommit = vi.fn().mockRejectedValue(Object.assign(new Error("Not Found"), { status: 404 }))
		const searchUsers = vi.fn().mockResolvedValue({
			data: {
				items: [
					{
						login: "octocat",
						avatar_url: "https://example.com/octocat-summary.png",
					},
				],
			},
		})
		const getByUsername = vi.fn().mockResolvedValue({
			data: {
				name: "The Octocat",
				login: "octocat",
				avatar_url: "https://example.com/octocat-profile.png",
			},
		})
		const service = createService({
			forgeName: "github",
			gitHubClient: {
				owner: "gitbutler",
				repo: "gitbutler",
				octokit: {
					rest: {
						repos: { getCommit },
						search: { users: searchUsers },
						users: {
							getAuthenticated: vi.fn().mockResolvedValue({
								data: {
									login: "someone-else",
									name: "Someone Else",
									email: "someone-else@example.com",
									avatar_url: "https://example.com/someone-else.png",
								},
							}),
							listEmailsForAuthenticatedUser: vi.fn().mockResolvedValue({
								data: [{ email: "someone-else@example.com" }],
							}),
							getByUsername,
						},
					},
				},
			},
		})

		const result = await service.resolve(
			{
				name: "Raw Commit Name",
				email: "octocat@example.com",
				gravatarUrl: "https://gravatar.example/octocat",
			},
			{ commitId: "local-only-commit" },
		)

		expect(result).toEqual({
			name: "The Octocat",
			email: "octocat@example.com",
			avatarUrl: "https://example.com/octocat-profile.png",
		})
		expect(getCommit).toHaveBeenCalledOnce()
		expect(searchUsers).toHaveBeenCalledOnce()
		expect(getByUsername).toHaveBeenCalledOnce()
	})

	test("uses the GitLab user lookup to enrich another author when available", async () => {
		const allUsers = vi.fn().mockResolvedValue([
			{
				name: "Jane Maintainer",
				avatar_url: "https://example.com/jane.png",
				public_email: "jane@example.com",
			},
		])
		const service = createService({
			forgeName: "gitlab",
			gitLabClient: {
				upstreamProjectId: "team/repo",
				api: {
					Users: {
						all: allUsers,
					},
				},
			},
		})

		const result = await service.resolve({
			name: "Raw Git Name",
			email: "jane@example.com",
			gravatarUrl: "https://gravatar.example/jane",
		})

		expect(result).toEqual({
			name: "Jane Maintainer",
			email: "jane@example.com",
			avatarUrl: "https://example.com/jane.png",
		})
		expect(allUsers).toHaveBeenCalledWith({ search: "jane@example.com" })
	})
})

function createService(args?: {
	forgeName?: DefaultForgeFactory["current"]["name"]
	gitHubClient?: MockGitHubClientInput
	gitLabClient?: MockGitLabClientInput
	user?: {
		email?: string
		name?: string
		picture?: string
	}
	gitConfigEmail?: string
}) {
	const gitConfigService = {
		get: vi.fn().mockResolvedValue(args?.gitConfigEmail),
	} as unknown as GitConfigService

	return new AuthorIdentityService({
		forgeFactory: {
			current: {
				name: args?.forgeName ?? "default",
			},
		} as unknown as DefaultForgeFactory,
		gitHubClient: {
			owner: undefined,
			repo: undefined,
			octokit: {
				rest: {
					repos: { getCommit: vi.fn() },
					search: { users: vi.fn() },
					users: {
						getAuthenticated: vi.fn().mockResolvedValue({
							data: {
								login: "test-user",
								name: "Test User",
								email: "test@example.com",
								avatar_url: "https://example.com/test-user.png",
							},
						}),
						listEmailsForAuthenticatedUser: vi.fn().mockResolvedValue({
							data: [{ email: "test@example.com" }],
						}),
						getByUsername: vi.fn(),
					},
				},
			},
			...args?.gitHubClient,
		} as unknown as GitHubClient,
		gitLabClient: {
			api: undefined,
			forkProjectId: undefined,
			upstreamProjectId: undefined,
			...args?.gitLabClient,
		} as unknown as GitLabClient,
		gitConfigService,
		userService: {
			user: args?.user,
		} as unknown as UserService,
	})
}
