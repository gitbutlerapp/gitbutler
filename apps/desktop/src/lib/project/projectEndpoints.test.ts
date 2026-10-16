import { ForgeInfoService } from "$lib/forge/forgeInfo.svelte";
import { buildGitEndpoints } from "$lib/git/gitEndpoints";
import { buildProjectEndpoints } from "$lib/project/projectEndpoints";
import { ReduxTag } from "$lib/state/tags";
import { configureStore } from "@reduxjs/toolkit";
import { createApi } from "@reduxjs/toolkit/query";
import { describe, expect, test } from "vitest";
import type { Project } from "$lib/project/project";
import type { BackendEndpointBuilder } from "$lib/state/backendApi";
import type { GitConfigSettings } from "@gitbutler/but-sdk";

function createEndpointBuilder(): BackendEndpointBuilder {
	return {
		mutation: (definition) => definition,
		query: (definition) => definition,
	} as BackendEndpointBuilder;
}

describe("project cache tags", () => {
	test("defines project query and mutation tags", () => {
		const endpoints = buildProjectEndpoints(createEndpointBuilder());
		const projectTags = endpoints.project.providesTags;
		const deleteTags = endpoints.deleteProject.invalidatesTags;
		const updateTags = endpoints.updateProject.invalidatesTags;
		if (
			typeof projectTags !== "function" ||
			typeof deleteTags !== "function" ||
			typeof updateTags !== "function"
		) {
			throw new Error("Expected project tag definitions to be callable");
		}

		expect(projectTags(undefined, undefined, { projectId: "project-1" }, undefined)).toEqual([
			{ type: ReduxTag.Project, id: "project-1" },
		]);
		expect(deleteTags(undefined, undefined, { projectId: "project-1" }, undefined)).toEqual([
			{ type: ReduxTag.Project, id: "LIST" },
		]);
		expect(
			updateTags(undefined, undefined, { project: { id: "project-1" } as Project }, undefined),
		).toEqual([
			{ type: ReduxTag.Project, id: "project-1" },
			{ type: ReduxTag.Project, id: "LIST" },
		]);
	});

	test("a null override does not invalidate forge queries", () => {
		const tags = buildProjectEndpoints(createEndpointBuilder()).updateProject.invalidatesTags;
		if (typeof tags !== "function") {
			throw new Error("Expected updateProject.invalidatesTags to be callable");
		}
		expect(
			tags(
				undefined,
				undefined,
				{ project: { id: "project-1", forge_override: null } as unknown as Project },
				undefined,
			),
		).toEqual([
			{ type: ReduxTag.Project, id: "project-1" },
			{ type: ReduxTag.Project, id: "LIST" },
		]);
	});

	test("setGbConfig invalidates its project item", () => {
		const endpoints = buildGitEndpoints(createEndpointBuilder());
		const tags = endpoints.setGbConfig.invalidatesTags;
		if (typeof tags !== "function") {
			throw new Error("Expected setGbConfig.invalidatesTags to be callable");
		}

		expect(
			tags(
				undefined,
				undefined,
				{ projectId: "project-1", config: {} as GitConfigSettings },
				undefined,
			),
		).toEqual([
			{ type: ReduxTag.GitButlerConfig, id: "project-1" },
			{ type: ReduxTag.Project, id: "project-1" },
		]);
	});

	test.each([{ forge_override: "gitlab" }, { unset_forge_override: true }])(
		"updating an override refreshes forge-dependent queries (%j)",
		(override) => {
			const endpoints = buildProjectEndpoints(createEndpointBuilder());
			const tags = endpoints.updateProject.invalidatesTags;
			if (typeof tags !== "function") {
				throw new Error("Expected updateProject.invalidatesTags to be callable");
			}

			expect(
				tags(
					undefined,
					undefined,
					{ project: { id: "project-1", ...override } as Project },
					undefined,
				),
			).toEqual([
				{ type: ReduxTag.Project, id: "project-1" },
				{ type: ReduxTag.Project, id: "LIST" },
				{ type: ReduxTag.ForgeProvider },
				{ type: ReduxTag.PullRequests },
				{ type: ReduxTag.RepoInfo },
				{ type: ReduxTag.Checks },
			]);
		},
	);

	test("setting and clearing an override refetches a subscribed null forge and compare URL", async () => {
		let forge: string | null = null;
		const api = createApi({
			reducerPath: "backend",
			tagTypes: Object.values(ReduxTag),
			baseQuery: async (args, _api, extra: { command?: string }) => {
				if (extra.command === "update_project") {
					forge = args.project.unset_forge_override ? null : args.project.forge_override;
					return { data: null };
				}
				return { data: forge };
			},
			endpoints: (build) => buildProjectEndpoints(build as BackendEndpointBuilder),
		});
		new ForgeInfoService(api as never);
		const store = configureStore({
			reducer: { [api.reducerPath]: api.reducer },
			middleware: (defaults) => defaults().concat(api.middleware),
		});
		const endpoints = api.endpoints as any;
		const queries = [
			[endpoints.forgeInfo, { projectId: "project-1" }],
			[
				endpoints.forgeCompareBranchUrl,
				{ projectId: "project-1", base: "main", branch: "topic", fork: null },
			],
		] as const;
		const subscriptions = queries.map(([endpoint, args]) =>
			store.dispatch(endpoint.initiate(args)),
		);
		await Promise.all(subscriptions);
		for (const [endpoint, args] of queries) {
			expect(endpoint.select(args)(store.getState()).data).toBeNull();
		}

		for (const override of [{ forge_override: "gitlab" }, { unset_forge_override: true }]) {
			await store.dispatch(
				endpoints.updateProject.initiate({ project: { id: "project-1", ...override } }),
			);
			await Promise.all(store.dispatch(api.util.getRunningQueriesThunk()));
			for (const [endpoint, args] of queries) {
				expect(endpoint.select(args)(store.getState()).data).toBe(forge);
			}
		}
		subscriptions.forEach((subscription) => subscription.unsubscribe());
		store.dispatch(api.util.resetApiState());
	});
});
