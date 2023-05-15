import { api } from '$lib';
import { asyncWritable } from '@square/svelte-store';

const projects = asyncWritable([], api.projects.list);

export default {
	...projects,
	add: (params: { path: string }) =>
		api.projects.add(params).then((project) => {
			projects.update((projects) => [...projects, project]);
			return project;
		})
};
