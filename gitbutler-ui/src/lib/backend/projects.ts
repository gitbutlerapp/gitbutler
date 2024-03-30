import { invoke } from '$lib/backend/ipc';
import { persisted } from '$lib/persisted/persisted';
import * as toasts from '$lib/utils/toasts';
import { open } from '@tauri-apps/api/dialog';
import { plainToInstance } from 'class-transformer';
import { get, writable, type Writable } from 'svelte/store';
import type { Project as CloudProject } from '$lib/backend/cloud';
import { goto } from '$app/navigation';

export type KeyType = 'default' | 'generated' | 'gitCredentialsHelper' | 'local';
export type LocalKey = {
	local: { private_key_path: string };
};

export type Key = Exclude<KeyType, 'local'> | LocalKey;

export class Project {
	id!: string;
	title!: string;
	description?: string;
	path!: string;
	api?: CloudProject & { sync: boolean };
	preferred_key!: Key;
	ok_with_force_push!: boolean;
	omit_certificate_check: boolean | undefined;
	use_diff_context: boolean | undefined;
}

export class ProjectService {
	private persistedId = persisted<string | undefined>(undefined, 'lastProject');
	projects: Writable<Project[]> = writable([]);
	error: Writable<undefined | any> = writable();

	constructor(private homeDir: string | undefined) {
		this.loadProjects();
	}

	async getProject(projectId: string) {
		return await invoke<Project>('get_project', { id: projectId });
	}

	/**
	 * Loads or reloads the list of projects
	 * @returns List of newly requested projects
	 */
	async loadProjects() {
		try {
			const projectObjects = await invoke<object[]>('list_projects');
			const projects = plainToInstance(Project, projectObjects);

			this.projects.set(projects);

			return projects;
		} catch (e) {
			this.projects.set([]);

			this.error.set(e);

			return [];
		}
	}

	async updateProject(params: {
		id: string;
		title?: string;
		api?: CloudProject & { sync: boolean };
		preferred_key?: Key;
		okWithForcePush?: boolean;
		omitCertificateCheck?: boolean;
	}) {
		await invoke<Project>('update_project', { project: params });

		this.loadProjects();
	}

	async add(path: string) {
		const project = await invoke<Project>('add_project', { path });
		await this.loadProjects();
		return project;
	}

	async deleteProject(id: string) {
		return await invoke('delete_project', { id });
	}

	async promptForDirectory(): Promise<string | undefined> {
		return open({ directory: true, recursive: true, defaultPath: this.homeDir }).then(
			(selectedPath) => {
				if (selectedPath === null) return;
				if (Array.isArray(selectedPath) && selectedPath.length !== 1) return;
				return Array.isArray(selectedPath) ? selectedPath[0] : selectedPath;
			}
		);
	}

	async addProject() {
		const path = await this.promptForDirectory();
		if (!path) return;
		return this.add(path)
			.then(async (project) => {
				if (!project) return;
				toasts.success(`Project ${project.title} created`);
				// linkProjectModal?.show(project.id);
				goto(`/${project.id}/board`);
			})
			.catch((e: any) => toasts.error(e.message));
	}

	getLastOpenedProject() {
		return get(this.persistedId);
	}

	setLastOpenedProject(projectId: string) {
		this.persistedId.set(projectId);
	}
}
