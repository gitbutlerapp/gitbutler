import { showError } from '$lib/notifications/toasts';
import { type AddProjectOutcome, type Project } from '$lib/project/project';
import { invalidatesList, providesItem, providesList, ReduxTag } from '$lib/state/tags';
import { getCookie } from '$lib/utils/cookies';
import { InjectionToken } from '@gitbutler/core/context';
import { persisted } from '@gitbutler/shared/persisted';
import { chipToasts } from '@gitbutler/ui';
import { get } from 'svelte/store';
import type { IBackend } from '$lib/backend';
import type { ClientState } from '$lib/state/clientState.svelte';

export type ProjectInfo = {
	is_exclusive: boolean;
	db_error?: string;
	headsup?: string;
};

export const PROJECTS_SERVICE = new InjectionToken<ProjectsService>('ProjectsService');

export class ProjectsService {
	private api: ReturnType<typeof injectEndpoints>;
	private persistedId = persisted<string | undefined>(undefined, 'lastProject');

	constructor(
		state: ClientState,
		private homeDir: string | undefined,
		private backend: IBackend
	) {
		this.api = injectEndpoints(state.backendApi);
	}

	projects() {
		return this.api.endpoints.listProjects.useQuery();
	}

	getProject(projectId: string, noValidation?: boolean) {
		return this.api.endpoints.project.useQuery({ projectId, noValidation });
	}

	async fetchProject(projectId: string, noValidation?: boolean) {
		return await this.api.endpoints.project.fetch({ projectId, noValidation });
	}

	async setActiveProject(projectId: string): Promise<ProjectInfo | null> {
		return await this.api.endpoints.setProjectActive.mutate({ id: projectId });
	}

	async updateProject(project: Project & { unset_bool?: boolean; unset_forge_override?: boolean }) {
		await this.api.endpoints.updateProject.mutate({ project });
	}

	async deleteProject(projectId: string) {
		return await this.api.endpoints.deleteProject.mutate({ projectId });
	}

	async promptForDirectory(): Promise<string | undefined> {
		const cookiePath = getCookie('test-projectPath');
		if (cookiePath) {
			return cookiePath;
		}
		const selectedPath = await this.backend.filePicker({
			directory: true,
			recursive: true,
			defaultPath: this.homeDir
		});
		if (selectedPath) {
			return selectedPath;
		}
	}

	// TODO: Reinstate the ability to open a project in a new window.
	async openProjectInNewWindow(projectId: string) {
		await this.api.endpoints.openProjectInWindow.mutate({ id: projectId });
	}

	async getCurrentProjectId(): Promise<string | undefined> {
		const result = await this.api.endpoints.getCurrentProjectId.query();
		return result || undefined;
	}

	async relocateProject(projectId: string): Promise<void> {
		const path = await this.getValidPath();
		if (!path) return;

		try {
			const project = await this.fetchProject(projectId, true);
			await this.updateProject({ ...project, path });
			chipToasts.success(`Project ${project.title} relocated`);
			window.location.reload();
		} catch (error: any) {
			showError('Failed to relocate project:', error.message);
		}
	}

	async addProject(path?: string) {
		if (!path) {
			path = await this.getValidPath();
			if (!path) return;
		}
		return await this.api.endpoints.addProject.mutate({ path });
	}

	async getValidPath(): Promise<string | undefined> {
		const path = await this.promptForDirectory();
		if (!path) return undefined;
		if (!this.validateProjectPath(path)) return undefined;
		return path;
	}

	validateProjectPath(path: string) {
		if (/^\\\\wsl.localhost/i.test(path)) {
			const errorMsg =
				'For WSL2 projects, install the Linux version of GitButler inside of your WSL2 distro';
			console.error(errorMsg);
			showError('Use the Linux version of GitButler', errorMsg);

			return false;
		}

		if (/^\\\\/i.test(path)) {
			const errorMsg =
				'Using git across a network is not recommended. Either clone ' +
				'the repo locally, or use the NET USE command to map a ' +
				'network drive';
			console.error(errorMsg);
			showError('UNC Paths are not directly supported', errorMsg);

			return false;
		}

		return true;
	}

	getLastOpenedProject() {
		return get(this.persistedId);
	}

	setLastOpenedProject(projectId: string) {
		this.persistedId.set(projectId);
	}

	unsetLastOpenedProject() {
		this.persistedId.set(undefined);
	}
}

function injectEndpoints(api: ClientState['backendApi']) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			listProjects: build.query<Project[], void>({
				extraOptions: { command: 'list_projects' },
				query: () => undefined,
				providesTags: [providesList(ReduxTag.Project)]
			}),
			project: build.query<Project, { projectId: string; noValidation?: boolean }>({
				extraOptions: { command: 'get_project' },
				query: (args) => args,
				providesTags: (_result, _error, args) => providesItem(ReduxTag.Project, args.projectId)
			}),
			addProject: build.mutation<AddProjectOutcome, { path: string }>({
				extraOptions: { command: 'add_project' },
				query: (args) => args,
				invalidatesTags: () => [invalidatesList(ReduxTag.Project)]
			}),
			deleteProject: build.mutation<Project[], { projectId: string }>({
				extraOptions: { command: 'delete_project' },
				query: (args) => args,
				invalidatesTags: () => [invalidatesList(ReduxTag.Project)]
			}),
			setProjectActive: build.mutation<ProjectInfo | null, { id: string }>({
				extraOptions: { command: 'set_project_active' },
				query: (args) => args
			}),
			updateProject: build.mutation<
				void,
				{ project: Project & { unset_bool?: boolean; unset_forge_override?: boolean } }
			>({
				extraOptions: { command: 'update_project' },
				query: (args) => args,
				invalidatesTags: (_result, _error, args) => providesItem(ReduxTag.Project, args.project.id)
			}),
			openProjectInWindow: build.mutation<void, { id: string }>({
				extraOptions: { command: 'open_project_in_window' },
				query: (args) => args
			}),
			getCurrentProjectId: build.query<string | null, void>({
				extraOptions: { command: 'get_current_project_id' },
				query: () => undefined
			})
		})
	});
}
