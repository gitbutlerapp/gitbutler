import { buildGitEndpoints } from "$lib/git/gitEndpoints";
import { buildProjectEndpoints } from "$lib/project/projectEndpoints";
import { ReduxTag } from "$lib/state/tags";
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
});
