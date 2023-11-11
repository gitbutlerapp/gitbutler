import { invoke } from '$lib/backend/ipc';
import type { Project as CloudProject } from '$lib/backend/cloud';
import { asyncWritable, derived, type Loadable } from '@square/svelte-store';

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

export function list() {
	return invoke<Project[]>('list_projects');
}

export function get(params: { id: string }): Promise<Project> {
	return invoke<Project>('get_project', params);
}

export function updateProject(params: {
	id: string;
	title?: string;
	api?: CloudProject & { sync: boolean };
	preferred_key?: Key;
}) {
	return invoke<Project>('update_project', { project: params });
}

export async function add(params: { path: string }) {
	return invoke<Project>('add_project', params);
}

export async function deleteProject(id: string) {
	return invoke('delete_project', { id }).then(() => {
		projectsStore.update((projects) => projects.filter((p) => p.id != id));
	});
}

export const projectsStore = asyncWritable([], list);

export function getProjectStore(id: string): Loadable<Project | undefined> {
	return {
		...derived(projectsStore, (projects) => projects?.find((p) => p.id === id))
	};
}

export async function addProject(path: string) {
	return add({ path }).then((project) => {
		projectsStore.update((projects) => [...projects, project]);
		return project;
	});
}
