import { invoke } from '$lib/backend/ipc';
import { showError } from '$lib/notifications/toasts';
import { persisted } from '$lib/persisted/persisted';
import { Project } from '$lib/projects/types';
import * as toasts from '$lib/utils/toasts';
import { open } from '@tauri-apps/api/dialog';
import { plainToInstance } from 'class-transformer';
import { get, writable } from 'svelte/store';
import type { HttpClient } from '../backend/httpClient';
import { goto } from '$app/navigation';

export class ProjectListingService {
	private persistedId = persisted<string | undefined>(undefined, 'lastProject');
	readonly projects = writable<Project[]>([], (set) => {
		this.loadAll()
			.then((projects) => {
				this.error.set(undefined);
				set(projects);
			})
			.catch((err) => {
				this.error.set(err);
				showError('Failed to load projects', err);
			});
	});
	readonly error = writable();

	constructor(
		private homeDir: string | undefined,
		private httpClient: HttpClient
	) {}

	private async loadAll() {
		return await invoke<Project[]>('list_projects').then((p) => plainToInstance(Project, p));
	}

	async reloadAll(): Promise<void> {
		this.projects.set(await this.loadAll());
	}

	async getProject(projectId: string) {
		return plainToInstance(Project, await invoke('get_project', { id: projectId }));
	}

	async updateProject(project: Project) {
		plainToInstance(Project, await invoke('update_project', { project: project }));
		this.reloadAll();
	}

	async add(path: string) {
		const project = plainToInstance(Project, await invoke('add_project', { path }));
		await this.reloadAll();
		return project;
	}

	async deleteProject(id: string) {
		return await invoke('delete_project', { id });
	}

	async promptForDirectory(): Promise<string | undefined> {
		const selectedPath = open({ directory: true, recursive: true, defaultPath: this.homeDir });
		if (selectedPath) {
			if (selectedPath === null) return;
			if (Array.isArray(selectedPath) && selectedPath.length !== 1) return;
			return Array.isArray(selectedPath) ? selectedPath[0] : await selectedPath;
		}
	}

	async addProject() {
		const path = await this.promptForDirectory();
		if (!path) return;

		if (!this.validateProjectPath(path)) return;

		try {
			const project = await this.add(path);
			if (!project) return;
			toasts.success(`Project ${project.title} created`);
			// linkProjectModal?.show(project.id);
			goto(`/${project.id}/board`);
		} catch (e: any) {
			showError('There was a problem', e.message);
		}
	}

	validateProjectPath(path: string, showErrors = true) {
		if (/^\\\\wsl.localhost/i.test(path)) {
			if (showErrors) {
				showError(
					'Use the Linux version of GitButler',
					'For WSL2 projects, install the Linux version of GitButler inside of your WSL2 distro'
				);
			}

			return false;
		}

		if (/^\\\\/i.test(path)) {
			if (showErrors) {
				showError(
					'UNC Paths are not directly supported',
					'Using git across a network is not recommended. Either clone the repo locally, or use the NET USE command to map a network drive'
				);
			}

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
}
