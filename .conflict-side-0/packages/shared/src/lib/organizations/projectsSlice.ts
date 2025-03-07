import { loadableUpsert, loadableUpsertMany } from '$lib/network/loadable';
import { createEntityAdapter, createSlice } from '@reduxjs/toolkit';
import type { LoadableProject } from '$lib/organizations/types';

const projectsAdapter = createEntityAdapter<LoadableProject, LoadableProject['id']>({
	selectId: (project: LoadableProject) => project.id
});

const projectsSlice = createSlice({
	name: 'projects',
	initialState: projectsAdapter.getInitialState(),
	reducers: {
		addProject: projectsAdapter.addOne,
		addProjects: projectsAdapter.addMany,
		removeProject: projectsAdapter.removeOne,
		removeProjects: projectsAdapter.removeMany,
		upsertProject: loadableUpsert(projectsAdapter),
		upsertProjects: loadableUpsertMany(projectsAdapter)
	}
});

export const projectsReducer = projectsSlice.reducer;

export const projectsSelectors = projectsAdapter.getSelectors();
export const {
	addProject,
	addProjects,
	removeProject,
	removeProjects,
	upsertProject,
	upsertProjects
} = projectsSlice.actions;
