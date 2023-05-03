import { invoke } from '$lib/ipc';
import type { Project as ApiProject } from '$lib/api/cloud';
import { asyncWritable, derived } from '@square/svelte-store';

export type Project = {
	id: string;
	title: string;
	path: string;
	api: ApiProject & { sync: boolean };
};

export const list = () => invoke<Project[]>('list_projects');

export const update = (params: {
	project: {
		id: string;
		title?: string;
		api?: ApiProject & { sync: boolean };
	};
}) => invoke<Project>('update_project', params);

export const add = (params: { path: string }) => invoke<Project>('add_project', params);

export const del = (params: { id: string }) => invoke('delete_project', params);

export const Projects = () => {
	const store = asyncWritable([], list);
	return {
		...store,
		get: async (id: string) => {
			await store.load();
			const project = derived(store, (projects) => {
				const project = projects.find((p) => p.id === id);
				if (!project) throw new Error(`Project ${id} not found`);
				return project;
			});
			return {
				...project,
				update: (params: { title?: string; api?: Project['api'] }) =>
					update({
						project: {
							id,
							...params
						}
					}).then((project) => {
						store.update((projects) => projects.map((p) => (p.id === project.id ? project : p)));
						return project;
					})
			};
		},
		add: (params: { path: string }) =>
			add(params).then((project) => {
				store.update((projects) => [...projects, project]);
				return project;
			}),
		delete: (params: { id: string }) =>
			del(params).then(() => {
				store.update((projects) => projects.filter((p) => p.id !== params.id));
			})
	};
};
