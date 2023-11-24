import { invoke } from '$lib/backend/ipc';
import type { Project as CloudProject } from '$lib/backend/cloud';
import { BehaviorSubject, catchError, from, map, shareReplay, switchMap } from 'rxjs';

export type Key =
	| 'generated'
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
};

export class ProjectService {
	private reload$ = new BehaviorSubject<void>(undefined);
	error$ = new BehaviorSubject<any>(undefined);
	projects$ = this.reload$.pipe(
		switchMap(() => from(invoke<Project[]>('list_projects'))),
		shareReplay(1),
		catchError((e) => {
			this.error$.next(e);
			return [];
		})
	);

	constructor() {}

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
	}) {
		await invoke<Project>('update_project', { project: params });
		this.reload();
	}

	async add(path: string) {
		const project = await invoke<Project>('add_project', { path });
		this.reload();
		return project;
	}

	async deleteProject(id: string) {
		await invoke('delete_project', { id });
		this.reload();
	}

	reload() {
		this.reload$.next();
	}
}
