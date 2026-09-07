import {
	gitLabEnterprisePatError,
	gitlabAccountIdentifierToString,
	injectBackendEndpoints,
	stringToGitLabAccountIdentifier,
} from "$lib/forge/gitlab/gitlabUserService.svelte";
import { ReduxTag } from "$lib/state/tags";
import { configureStore } from "@reduxjs/toolkit";
import { createApi, QueryStatus } from "@reduxjs/toolkit/query";
import { describe, expect, test, vi } from "vitest";
import type { BackendApi } from "$lib/state/backendApi";
import type { GitlabAccountIdentifier } from "@gitbutler/but-sdk";

const UNIT_SEP = "\u001F";

describe("storing a replacement PAT", () => {
	const account: GitlabAccountIdentifier = {
		type: "selfHosted",
		info: { host: "gitlab.example.com", username: "alice" },
	};
	const user = { accessToken: "new", username: "alice", avatarUrl: null, name: null, email: null };

	const rejected = { error: { code: "GitLabUnauthorized", message: "rejected" } };

	/**
	 * The service's real endpoint definitions on a plain RTK store. The user
	 * lookup is rejected once, the way a revoked token is, and answered after;
	 * every other command answers with `storeOutcome`.
	 */
	function setup(storeOutcome: { data: unknown } | { error: unknown }) {
		const lookups = vi.fn().mockResolvedValueOnce(rejected).mockResolvedValue({ data: user });
		const api = createApi({
			reducerPath: "backend",
			tagTypes: Object.values(ReduxTag),
			invalidationBehavior: "immediately",
			baseQuery: async (_args: unknown, _api: unknown, extraOptions?: { command?: string }) =>
				extraOptions?.command === "get_gl_user" ? lookups() : storeOutcome,
			endpoints: () => ({}),
		});
		const { endpoints } = injectBackendEndpoints(api as unknown as BackendApi);
		const store = configureStore({
			reducer: { [api.reducerPath]: api.reducer },
			middleware: (getDefault) => getDefault().concat(api.middleware),
		});
		function userQuery() {
			return endpoints.getGitLabUser.select({ account })(store.getState());
		}
		return { endpoints, store, userQuery, lookups };
	}
	type Harness = ReturnType<typeof setup>;

	/** Subscribes the user query, as a mounted settings card does, and lets it reject. */
	async function mountRejectedUserQuery({ store, endpoints, userQuery }: Harness) {
		await store.dispatch(endpoints.getGitLabUser.initiate({ account }));
		expect(userQuery().status).toBe(QueryStatus.rejected);
	}

	const mutations: [string, (harness: Harness) => Promise<void>][] = [
		[
			"storeGitLabPat",
			async ({ store, endpoints }) => {
				await store.dispatch(endpoints.storeGitLabPat.initiate({ accessToken: "new" }));
			},
		],
		[
			"storeGitLabEnterprisePat",
			async ({ store, endpoints }) => {
				await store.dispatch(
					endpoints.storeGitLabEnterprisePat.initiate({
						host: "gitlab.example.com",
						accessToken: "new",
					}),
				);
			},
		],
	];

	test.each(mutations)(
		"%s refetches a still-mounted rejected getGitLabUser query",
		async (_, storePat) => {
			const harness = setup({ data: {} });
			await mountRejectedUserQuery(harness);

			await storePat(harness);

			await vi.waitFor(() => expect(harness.userQuery().status).toBe(QueryStatus.fulfilled));
			expect(harness.userQuery().data).toEqual(user);
			expect(harness.lookups).toHaveBeenCalledTimes(2);
		},
	);

	test.each(mutations)(
		"%s leaves the rejected query alone when the token was not stored",
		async (_, storePat) => {
			const harness = setup(rejected);
			await mountRejectedUserQuery(harness);

			await storePat(harness);

			// An invalidation refetch would already have started by now.
			await new Promise((resolve) => setTimeout(resolve, 20));
			expect(harness.lookups).toHaveBeenCalledTimes(1);
			expect(harness.userQuery().status).toBe(QueryStatus.rejected);
		},
	);
});

describe("GitLab Enterprise PAT errors", () => {
	test.each([
		[
			"GitLabUnauthorized",
			"The token was not accepted. Check that it is correct, active, and issued by this GitLab host.",
		],
		[
			"GitLabForbidden",
			"GitLab refused access. Check the token scopes and account permissions, or ask your administrator about instance policies.",
		],
		["NetworkError", "Invalid token or host"],
		["Unknown", "Invalid token or host"],
	])("maps %s to cautious guidance", (code, expected) => {
		expect(gitLabEnterprisePatError({ code, message: "backend detail" })).toBe(expected);
	});
});

describe("gitlab account identifier serialization", () => {
	test("serializes a patUsername account", () => {
		const account: GitlabAccountIdentifier = {
			type: "patUsername",
			info: {
				username: "octocat",
			},
		};

		expect(gitlabAccountIdentifierToString(account)).toBe(`patUsername${UNIT_SEP}octocat`);
	});

	test("serializes a selfHosted account", () => {
		const account: GitlabAccountIdentifier = {
			type: "selfHosted",
			info: {
				host: "gitlab.example.com",
				username: "alice",
			},
		};

		expect(gitlabAccountIdentifierToString(account)).toBe(
			`selfHosted${UNIT_SEP}gitlab.example.com${UNIT_SEP}alice`,
		);
	});
});

describe("gitlab account identifier deserialization", () => {
	test("deserializes a patUsername account", () => {
		expect(stringToGitLabAccountIdentifier(`patUsername${UNIT_SEP}octocat`)).toStrictEqual({
			type: "patUsername",
			info: {
				username: "octocat",
			},
		});
	});

	test("deserializes a selfHosted account", () => {
		expect(
			stringToGitLabAccountIdentifier(`selfHosted${UNIT_SEP}gitlab.example.com${UNIT_SEP}alice`),
		).toStrictEqual({
			type: "selfHosted",
			info: {
				host: "gitlab.example.com",
				username: "alice",
			},
		});
	});

	test("returns null for malformed or unsupported values", () => {
		expect(stringToGitLabAccountIdentifier("patUsername")).toBeNull();
		expect(stringToGitLabAccountIdentifier("enterprise\u001Fhost\u001Fuser")).toBeNull();
		expect(stringToGitLabAccountIdentifier(`selfHosted${UNIT_SEP}only-host`)).toBeNull();
	});
});

describe("gitlab account identifier round-trip", () => {
	test("round-trips supported account types", () => {
		const accounts: GitlabAccountIdentifier[] = [
			{
				type: "patUsername",
				info: {
					username: "octocat",
				},
			},
			{
				type: "selfHosted",
				info: {
					host: "gitlab.example.com",
					username: "alice",
				},
			},
		];

		for (const account of accounts) {
			const serialized = gitlabAccountIdentifierToString(account);
			expect(stringToGitLabAccountIdentifier(serialized)).toStrictEqual(account);
		}
	});
});
