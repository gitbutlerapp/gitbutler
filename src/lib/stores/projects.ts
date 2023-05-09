import { asyncWritable, derived } from '@square/svelte-store';
import { api } from '$lib';

const store = asyncWritable([], api.projects.list);

export const list = () => store;

export const get = (id: string) => derived(store, (projects) => projects.find((p) => p.id === id));

export const update = async (project: { id: string; title?: string; api?: api.Project['api'] }) => {
	const updated = await api.projects.update({ project });
	store.update((projects) => {
		const index = projects.findIndex((p) => p.id === project.id);
		if (index === -1) {
			return [...projects, updated];
		} else {
			projects[index] = updated;
			return projects;
		}
	});
	return updated;
};

export const del = async (project: { id: string }) => {
	await api.projects.del(project);
	store.update((projects) => projects.filter((p) => p.id !== project.id));
};

export const add = async (params: { path: string }) => {
	const project = await api.projects.add(params);
	store.update((projects) => [...projects, project]);
	return project;
};
