import { invoke } from '$lib/backend/ipc';
import { showError } from '$lib/notifications/toasts';
import { type Project } from '$lib/project/project';
import { invalidatesList, providesItem, providesList, ReduxTag } from '$lib/state/tags';
import { InjectionToken } from '@gitbutler/shared/context';
import { persisted } from '@gitbutler/shared/persisted';
import { chipToasts } from '@gitbutler/ui';
import { open } from '@tauri-apps/plugin-dialog';
import { get } from 'svelte/store';
import type { ClientState } from '$lib/state/clientState.svelte';
import type { HttpClient } from '@gitbutler/shared/network/httpClient';

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
		private httpClient: HttpClient
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
		const info = await invoke<ProjectInfo | null>('set_project_active', { id: projectId });
		return info;
	}

	async updateProject(project: Project & { unset_bool?: boolean; unset_forge_override?: boolean }) {
		await invoke('update_project', { project: project });
	}

	async deleteProject(projectId: string) {
		return await this.api.endpoints.deleteProject.mutate({ projectId });
	}

	async promptForDirectory(): Promise<string | undefined> {
		const selectedPath = open({ directory: true, recursive: true, defaultPath: this.homeDir });
		if (selectedPath) {
			if (selectedPath === null) return;
			if (Array.isArray(selectedPath) && selectedPath.length !== 1) return;
			return Array.isArray(selectedPath) ? selectedPath[0] : ((await selectedPath) ?? undefined);
		}
	}

	// TODO: Reinstate the ability to open a project in a new window.
	async openProjectInNewWindow(projectId: string) {
		await invoke('open_project_in_window', { id: projectId });
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
			addProject: build.mutation<Project, { path: string }>({
				extraOptions: { command: 'add_project' },
				query: (args) => args,
				invalidatesTags: () => [invalidatesList(ReduxTag.Project)]
			}),
			deleteProject: build.mutation<Project[], { projectId: string }>({
				extraOptions: { command: 'delete_project' },
				query: (args) => args,
				invalidatesTags: () => [invalidatesList(ReduxTag.Project)]
			})
		})
	});
}
