import { invalidatesList, providesItem, providesList, ReduxTag } from "$lib/state/tags";
import type { TreeChanges } from "$lib/hunks/change";
import type { AddProjectOutcome, Project } from "$lib/project/project";
import type { BackendEndpointBuilder } from "$lib/state/backendApi";

export type ProjectInfo = {
	is_exclusive: boolean;
	db_error?: string;
	headsup?: string;
};

export function buildProjectEndpoints(build: BackendEndpointBuilder) {
	return {
		// ── Projects ────────────────────────────────────────────────
		listProjects: build.query<Project[], void>({
			extraOptions: { command: "list_projects" },
			query: () => undefined,
			providesTags: [providesList(ReduxTag.Project)],
		}),
		project: build.query<Project, { projectId: string; noValidation?: boolean }>({
			extraOptions: { command: "get_project" },
			query: (args) => args,
			providesTags: (_result, _error, args) => providesItem(ReduxTag.Project, args.projectId),
		}),
		addProject: build.mutation<AddProjectOutcome, { path: string }>({
			extraOptions: { command: "add_project" },
			query: (args) => args,
			invalidatesTags: () => [invalidatesList(ReduxTag.Project)],
		}),
		addProjectWithBestEffort: build.mutation<AddProjectOutcome, { path: string }>({
			extraOptions: { command: "add_project_best_effort" },
			query: (args) => args,
			invalidatesTags: () => [invalidatesList(ReduxTag.Project)],
		}),
		deleteProject: build.mutation<Project[], { projectId: string }>({
			extraOptions: { command: "delete_project" },
			query: (args) => args,
			invalidatesTags: () => [invalidatesList(ReduxTag.Project)],
		}),
		setProjectActive: build.mutation<ProjectInfo | null, { id: string }>({
			extraOptions: { command: "set_project_active" },
			query: (args) => args,
		}),
		updateProject: build.mutation<
			void,
			{ project: Project & { unset_bool?: boolean; unset_forge_override?: boolean } }
		>({
			extraOptions: { command: "update_project" },
			query: (args) => args,
			invalidatesTags: (_result, _error, args) => providesItem(ReduxTag.Project, args.project.id),
		}),
		openProjectInWindow: build.mutation<void, { id: string }>({
			extraOptions: { command: "open_project_in_window" },
			query: (args) => args,
		}),
		areYouGerritKiddingMe: build.query<boolean, { projectId: string }>({
			extraOptions: { command: "is_gerrit" },
			query: (args) => args,
			providesTags: (_result, _error, args) => providesItem(ReduxTag.ProjectGerrit, args.projectId),
		}),

		// ── Oplog ───────────────────────────────────────────────────
		oplogDiffWorktrees: build.query<TreeChanges, { projectId: string; snapshotId: string }>({
			extraOptions: { command: "oplog_diff_worktrees" },
			query: (args) => args,
		}),
	};
}
