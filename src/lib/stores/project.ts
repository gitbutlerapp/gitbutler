import { api } from '$lib';
import { derived } from '@square/svelte-store';
import type { Project } from '$lib/api';
import projects from './projects';

export default ({ id }: { id: string }) => ({
	...derived(projects, (projects) => projects?.find((p) => p.id === id)),
	update: (params: Partial<Pick<Project, 'title' | 'description' | 'api'>>) =>
		api.projects
			.update({
				project: {
					id,
					...params
				}
			})
			.then((project) => {
				projects.update((projects) => projects.map((p) => (p.id === project.id ? project : p)));
				return project;
			}),
	delete: () =>
		api.projects
			.delete({ id })
			.then(() => projects.update((projects) => projects.filter((p) => p.id !== id)))
});
