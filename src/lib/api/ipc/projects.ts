import { invoke } from '$lib/ipc';
import type { Project as CloudProject } from '$lib/api/cloud';
import { asyncWritable, derived } from '@square/svelte-store';

export type Project = {
	id: string;
	title: string;
	description?: string;
	path: string;
	api?: CloudProject & { sync: boolean };
};

export function list() {
	return invoke<Project[]>('list_projects');
}

export function get(params: { id: string }): Promise<Project> {
	return invoke<Project>('get_project', params);
}

export function update(params: {
	project: {
		id: string;
		title?: string;
		api?: CloudProject & { sync: boolean };
	};
}) {
	return invoke<Project>('update_project', params);
}

export function add(params: { path: string }) {
	return invoke<Project>('add_project', params);
}

export function del(params: { id: string }) {
	return invoke('delete_project', params);
}

const store = asyncWritable([], list);

export function getProjectStore({ id }: { id: string }) {
	return {
		...derived(store, (projects) => projects?.find((p) => p.id === id)),
		update: (params: Partial<Pick<Project, 'title' | 'description' | 'api'>>) =>
			update({
				project: {
					id,
					...params
				}
			}).then((project) => {
				store.update((projects) => projects.map((p) => (p.id === project.id ? project : p)));
				return project;
			}),
		delete: () =>
			del({ id }).then(() => store.update((projects) => projects.filter((p) => p.id !== id)))
	};
}

export const projectsStore = {
	...store,
	add: (params: { path: string }) =>
		add(params).then((project) => {
			store.update((projects) => [...projects, project]);
			return project;
		})
};
