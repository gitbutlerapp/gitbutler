import { invoke } from '$lib/ipc';
import type { Project as CloudProject } from '$lib/api/cloud';

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

const del = (params: { id: string }) => invoke('delete_project', params);
export { del as delete };
