import { invoke } from '@tauri-apps/api';
import { derived, writable } from 'svelte/store';
import type { Project as ApiProject } from '$lib/api';

export type Project = {
	id: string;
	title: string;
	path: string;
	api: ApiProject & { sync: boolean };
};

const list = () => invoke<Project[]>('list_projects');

const update = (params: {
	project: {
		id: string;
		title?: string;
		api?: ApiProject & { sync: boolean };
	};
}) => invoke<Project>('update_project', params);

const add = (params: { path: string }) => invoke<Project>('add_project', params);

const del = (params: { id: string }) => invoke('delete_project', params);

export default async () => {
	const init = await list();
	const store = writable<Project[]>(init);

	return {
		subscribe: store.subscribe,
		get: (id: string) => {
			const project = derived(store, (projects) => {
				const project = projects.find((p) => p.id === id);
				if (!project) throw new Error(`Project ${id} not found`);
				return project;
			});
			return {
				subscribe: project.subscribe,
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
