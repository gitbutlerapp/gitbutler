import { invoke } from '$lib/backend/ipc';
import type { Project as CloudProject } from '$lib/backend/cloud';
import { open } from '@tauri-apps/api/dialog';
import { goto } from '$app/navigation';
import * as toasts from '$lib/utils/toasts';
import {
	BehaviorSubject,
	catchError,
	firstValueFrom,
	from,
	map,
	shareReplay,
	skip,
	switchMap
} from 'rxjs';
import { persisted } from '$lib/persisted/persisted';
import { get } from 'svelte/store';

export type Key =
	| 'default'
	| 'generated'
	| 'gitCredentialsHelper'
	| {
			local: { private_key_path: string; passphrase?: string };
	  };

export type Project = {
	id: string;
	title: string;
	description?: string;
	path: string;
	api?: CloudProject & { sync: boolean };
	preferred_key: Key;
	ok_with_force_push: boolean;
};

export class ProjectService {
	private reload$ = new BehaviorSubject<void>(undefined);
	private persistedId = persisted<string | undefined>(undefined, 'lastProject');
	error$ = new BehaviorSubject<any>(undefined);

	projects$ = this.reload$.pipe(
		switchMap(() => from(invoke<Project[]>('list_projects'))),
		shareReplay(1),
		catchError((e) => {
			this.error$.next(e);
			return [];
		})
	);

	constructor(private homeDir: string | undefined) {}

	getProject(projectId: string) {
		return this.projects$.pipe(
			map((projects) => {
				const project = projects.find((p) => p.id == projectId);
				if (!project) {
					// We need to abort loading of /[projectId]/ if no project exists, such
					// that the type here is of Project rather than Project | undefined.
					throw 'Project not found';
				}
				return project;
			})
		);
	}

	async updateProject(params: {
		id: string;
		title?: string;
		api?: CloudProject & { sync: boolean };
		preferred_key?: Key;
		okWithForcePush?: boolean;
	}) {
		await invoke<Project>('update_project', { project: params });
		this.reload();
	}

	async add(path: string) {
		const project = await invoke<Project>('add_project', { path });
		await this.reload();
		return project;
	}

	async deleteProject(id: string) {
		await invoke('delete_project', { id });
		this.reload();
	}

	async reload(): Promise<Project[]> {
		const projects = firstValueFrom(this.projects$.pipe(skip(1)));
		this.reload$.next();
		return projects;
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
