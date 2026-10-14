import { useForgeAuth } from "$lib/forge/forgeAuth.svelte";
import { useGiteeForgeUser } from "$lib/forge/gitee/hooks.svelte";
import { effect_root } from "svelte/internal/client";
import { describe, expect, test, vi } from "vitest";

const state = vi.hoisted(() => ({
	accounts: [] as unknown[],
	user: null as null | { username: string; name: null; avatarUrl: null },
	loading: false,
	error: false,
}));
vi.mock("@gitbutler/core/context", () => ({ inject: (token: unknown) => token }));
vi.mock("@gitbutler/shared/reactiveUtils.svelte", () => ({
	reactive: (read: () => unknown) => ({
		get current() {
			return read();
		},
	}),
}));
vi.mock("$lib/forge/forgeInfo.svelte", () => ({
	FORGE_INFO_SERVICE: { get: () => ({ response: { name: "gitee" } }) },
}));
vi.mock("$lib/project/projectsService", () => ({
	PROJECTS_SERVICE: { getProject: () => ({ response: { preferred_forge_user: null } }) },
}));
vi.mock("$lib/forge/gitee/giteeUserService.svelte", () => ({
	isSameGiteeAccountIdentifier: () => true,
	GITEE_USER_SERVICE: {
		accounts: () => ({ response: state.accounts }),
		authenticatedUser: (
			_account: unknown,
			options: { transform: (value: unknown) => unknown },
		) => ({
			response: options.transform(state.user),
			result: { isLoading: state.loading, isError: state.error },
		}),
	},
}));
vi.mock("$lib/forge/github/hooks.svelte", () => ({
	useGitHubForgeUser: () => ({ user: { current: undefined }, isLoading: { current: false } }),
}));
vi.mock("$lib/forge/gitlab/hooks.svelte", () => ({
	useGitLabForgeUser: () => ({ user: { current: undefined }, isLoading: { current: false } }),
}));
vi.mock("$lib/forge/bitbucket/hooks.svelte", () => ({
	useBitbucketForgeUser: () => ({ user: { current: undefined }, isLoading: { current: false } }),
}));

describe("Gitee forge authentication", () => {
	test.each([
		{
			title: "no account",
			account: false,
			user: false,
			error: false,
			loading: false,
			authenticated: false,
		},
		{
			title: "verified PAT",
			account: true,
			user: true,
			error: false,
			loading: false,
			authenticated: true,
		},
		{
			title: "rejected PAT with stale user data",
			account: true,
			user: true,
			error: true,
			loading: false,
			authenticated: false,
		},
		{
			title: "loading account",
			account: true,
			user: false,
			error: false,
			loading: true,
			authenticated: false,
		},
	])("$title", (scenario) => {
		state.accounts = scenario.account ? [{ type: "patUsername", info: { username: "alice" } }] : [];
		state.user = scenario.user ? { username: "alice", name: null, avatarUrl: null } : null;
		state.error = scenario.error;
		state.loading = scenario.loading;
		const destroy = effect_root(() => {
			const auth = useForgeAuth({ current: "project" });
			expect(auth.authenticated.current).toBe(scenario.authenticated);
			expect(auth.isLoading.current).toBe(scenario.loading);
			const user = useGiteeForgeUser({ current: "project" });
			expect(user.user.current).toEqual(
				scenario.authenticated ? { login: "alice", name: "alice", srcUrl: "" } : undefined,
			);
		});
		destroy();
	});
});
