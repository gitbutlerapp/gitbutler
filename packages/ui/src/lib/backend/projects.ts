import { invoke } from '$lib/backend/ipc';
import type { Project as CloudProject } from '$lib/backend/cloud';
import { BehaviorSubject, catchError, from, switchMap } from 'rxjs';

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
		catchError((e) => {
			this.error$.next(e);
			return [];
		})
	);

	constructor() {}

	getProject(projectId: string) {
		return from(invoke<Project>('get_project', { id: projectId }));
	}

	updateProject(params: {
		id: string;
		title?: string;
		api?: CloudProject & { sync: boolean };
		preferred_key?: Key;
	}) {
		return invoke<Project>('update_project', { project: params });
	}

	async add(path: string) {
		return await invoke<Project>('add_project', { path });
	}

	async deleteProject(id: string) {
		await invoke('delete_project', { id });
		this.reload();
	}

	reload() {
		this.reload$.next();
	}
}
