import { buildBranchEndpoints } from "$lib/branches/branchEndpoints";
import { invalidatesList, invalidatesType, providesType, ReduxTag } from "$lib/state/tags";
import { describe, expect, test } from "vitest";
import type { BackendEndpointBuilder } from "$lib/state/backendApi";

function createEndpointBuilder(): BackendEndpointBuilder {
	return {
		mutation: (definition) => definition,
		query: (definition) => definition,
	} as BackendEndpointBuilder;
}

describe("buildBranchEndpoints", () => {
	test("maps fetch APIs to the workspace-prefixed commands", () => {
		const endpoints = buildBranchEndpoints(createEndpointBuilder());

		expect(endpoints.workspaceFetchStatus.extraOptions).toEqual({
			command: "workspace_fetch_status",
		});
		expect(endpoints.workspaceFetchStatus.query?.({ projectId: "project-1" })).toEqual({
			projectId: "project-1",
		});
		expect(endpoints.workspaceFetchStatus.providesTags).toEqual([
			providesType(ReduxTag.WorkspaceFetchStatus),
		]);

		expect(endpoints.workspaceFetchFromRemotes.extraOptions).toEqual({
			command: "workspace_fetch_from_remotes",
		});
		expect(
			endpoints.workspaceFetchFromRemotes.query?.({
				projectId: "project-1",
				action: "modal",
			}),
		).toEqual({
			projectId: "project-1",
			action: "modal",
		});
		expect(endpoints.workspaceFetchFromRemotes.invalidatesTags).toEqual([
			invalidatesType(ReduxTag.WorkspaceFetchStatus),
			invalidatesList(ReduxTag.Stacks),
			invalidatesList(ReduxTag.StackDetails),
		]);
	});

	test("names the switch-back mutation", () => {
		const endpoints = buildBranchEndpoints(createEndpointBuilder());

		expect(endpoints.switchBackToWorkspace.extraOptions).toEqual({
			command: "switch_back_to_workspace",
			actionName: "Switch Back to Workspace",
		});
	});

	test("defaults workspace fetch action to auto", () => {
		const endpoints = buildBranchEndpoints(createEndpointBuilder());

		expect(
			endpoints.workspaceFetchFromRemotes.query?.({
				projectId: "project-1",
			}),
		).toEqual({
			projectId: "project-1",
			action: "auto",
		});
	});
});
