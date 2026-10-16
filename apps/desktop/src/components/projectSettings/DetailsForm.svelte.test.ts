import DetailsForm from "$components/projectSettings/DetailsForm.svelte";
import { buildProjectEndpoints } from "$lib/project/projectEndpoints";
import { PROJECTS_SERVICE } from "$lib/project/projectsService";
import { ReduxTag } from "$lib/state/tags";
import { configureStore } from "@reduxjs/toolkit";
import { createApi } from "@reduxjs/toolkit/query";
import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, expect, test, vi } from "vitest";
import type { Project } from "$lib/project/project";
import type { BackendEndpointBuilder } from "$lib/state/backendApi";

afterEach(() => {
	cleanup();
	vi.unstubAllGlobals();
});

test.each(["gitlab", null, undefined])(
	"editing a description leaves forge queries cached with override %s",
	async (forgeOverride) => {
		vi.stubGlobal(
			"ResizeObserver",
			class {
				observe() {}
				unobserve() {}
				disconnect() {}
			},
		);
		const project = {
			id: "project-1",
			title: "Project",
			path: "/repo",
			description: "",
			forge_override: forgeOverride,
		} as Project;
		const projectsService = {
			getProject: () => ({ result: { data: project } }),
			updateProject: vi.fn(),
		};
		const forgeTags = [
			ReduxTag.ForgeProvider,
			ReduxTag.PullRequests,
			ReduxTag.RepoInfo,
			ReduxTag.Checks,
		];
		const fetchForge = vi.fn(() => ({ data: "cached" }));
		const api = createApi({
			reducerPath: "backend",
			tagTypes: Object.values(ReduxTag),
			baseQuery: async (_args, _api, extra: { command?: string }) =>
				extra?.command === "update_project" ? { data: null } : fetchForge(),
			endpoints: (build) => ({
				...buildProjectEndpoints(build as BackendEndpointBuilder),
				forgeData: build.query<string, ReduxTag>({
					query: (tag) => tag,
					providesTags: (_result, _error, tag) => [{ type: tag }],
				}),
			}),
		});
		const store = configureStore({
			reducer: { [api.reducerPath]: api.reducer },
			middleware: (defaults) => defaults().concat(api.middleware),
		});
		projectsService.updateProject.mockImplementation(async (update) => {
			await store.dispatch(api.endpoints.updateProject.initiate({ project: update }));
		});
		try {
			await Promise.all(
				forgeTags.map(async (tag) => await store.dispatch(api.endpoints.forgeData.initiate(tag))),
			);
			expect(fetchForge).toHaveBeenCalledTimes(4);
			render(DetailsForm, {
				props: { projectId: project.id },
				context: new Map([[PROJECTS_SERVICE._key, projectsService]]),
			});
			const description = screen.getByPlaceholderText("Project description");
			for (const value of ["a", "ab", "abc"]) {
				await fireEvent.input(description, { target: { value } });
				await Promise.all(projectsService.updateProject.mock.results.map(({ value }) => value));
				await Promise.all(store.dispatch(api.util.getRunningQueriesThunk()));
				expect(fetchForge).toHaveBeenCalledTimes(4);
				expect(projectsService.updateProject).toHaveBeenLastCalledWith({
					id: project.id,
					description: value,
				});
			}
		} finally {
			store.dispatch(api.util.resetApiState());
		}
	},
);
