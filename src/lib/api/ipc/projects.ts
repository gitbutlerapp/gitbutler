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

export const list = () => invoke<Project[]>('list_projects');

export const update = (params: {
	project: {
		id: string;
		title?: string;
		api?: CloudProject & { sync: boolean };
	};
}) => invoke<Project>('update_project', params);

export const add = (params: { path: string }) => invoke<Project>('add_project', params);

export const del = (params: { id: string }) => invoke('delete_project', params);

const store = asyncWritable([], list);

export const Project = ({ id }: { id: string }) => ({
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
});

export const Projects = () => ({
	...store,
	add: (params: { path: string }) =>
		add(params).then((project) => {
			store.update((projects) => [...projects, project]);
			return project;
		})
});
