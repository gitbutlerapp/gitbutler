import { invoke } from '$lib/backend/ipc';
import { showError } from '$lib/notifications/toasts';
import { persisted } from '$lib/persisted/persisted';
import * as toasts from '$lib/utils/toasts';
import { open } from '@tauri-apps/api/dialog';
import { plainToInstance } from 'class-transformer';
import { get, writable } from 'svelte/store';
import type { HttpClient } from './httpClient';
import { goto } from '$app/navigation';

export type KeyType = 'gitCredentialsHelper' | 'local' | 'systemExecutable';
export type LocalKey = {
	local: { private_key_path: string };
};

export type Key = Exclude<KeyType, 'local'> | LocalKey;

export class Project {
	id!: string;
	title!: string;
	description?: string;
	path!: string;
	api?: CloudProject & { sync: boolean; sync_code: boolean | undefined };
	preferred_key!: Key;
	ok_with_force_push!: boolean;
	omit_certificate_check: boolean | undefined;
	use_diff_context: boolean | undefined;
	snapshot_lines_threshold!: number | undefined;
	use_new_locking!: boolean;
	git_host!: {
		hostType: 'github' | 'gitlab' | 'bitbucket' | 'azure';
		pullRequestTemplatePath: string;
	};

	private succeeding_rebases!: boolean;
	get succeedingRebases() {
		return this.succeeding_rebases;
	}
	set succeedingRebases(value) {
		this.succeeding_rebases = value;
	}

	// Produced just for the frontend to determine if the project is open in any window.
	is_open!: boolean;

	get vscodePath() {
		return this.path.includes('\\') ? '/' + this.path.replace('\\', '/') : this.path;
	}
}

export type CloudProject = {
	name: string;
	description: string | null;
	repository_id: string;
	git_url: string;
	created_at: string;
	updated_at: string;
};

export class ProjectService {
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

	async reload(): Promise<void> {
		this.projects.set(await this.loadAll());
	}

	async getProject(projectId: string, noValidation?: boolean) {
		return plainToInstance(Project, await invoke('get_project', { id: projectId, noValidation }));
	}

	async updateProject(project: Project) {
		plainToInstance(Project, await invoke('update_project', { project: project }));
		this.reload();
	}

	async add(path: string) {
		const project = plainToInstance(Project, await invoke('add_project', { path }));
		await this.reload();
		return project;
	}

	async deleteProject(id: string) {
		await invoke('delete_project', { id });
		await this.reload();
	}

	async promptForDirectory(): Promise<string | undefined> {
		const selectedPath = open({ directory: true, recursive: true, defaultPath: this.homeDir });
		if (selectedPath) {
			if (selectedPath === null) return;
			if (Array.isArray(selectedPath) && selectedPath.length !== 1) return;
			return Array.isArray(selectedPath) ? selectedPath[0] : await selectedPath;
		}
	}

	async openProjectInNewWindow(projectId: string) {
		await invoke('open_project_in_window', { id: projectId });
	}

	async relocateProject(projectId: string): Promise<void> {
		const path = await this.getValidPath();
		if (!path) return;

		try {
			const project = await this.getProject(projectId, true);
			project.path = path;
			await this.updateProject(project);
			toasts.success(`Project ${project.title} relocated`);

			goto(`/${project.id}/board`);
		} catch (error: any) {
			showError('Failed to relocate project:', error.message);
		}
	}

	async addProject(path?: string) {
		if (!path) {
			path = await this.getValidPath();
			if (!path) return;
		}

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
				'Using git across a network is not recommended. Either clone the repo locally, or use the NET USE command to map a network drive';
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

	async createCloudProject(
		token: string,
		params: {
			name: string;
			description?: string;
			uid?: string;
		}
	): Promise<CloudProject> {
		return await this.httpClient.post('projects.json', {
			body: params,
			token
		});
	}

	async updateCloudProject(
		token: string,
		repositoryId: string,
		params: {
			name: string;
			description?: string;
		}
	): Promise<CloudProject> {
		return await this.httpClient.put(`projects/${repositoryId}.json`, {
			body: params,
			token
		});
	}

	async getCloudProject(token: string, repositoryId: string): Promise<CloudProject> {
		return await this.httpClient.get(`projects/${repositoryId}.json`, {
			token
		});
	}
}
