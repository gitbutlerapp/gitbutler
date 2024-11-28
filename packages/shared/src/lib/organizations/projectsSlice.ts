import { createEntityAdapter, createSlice } from '@reduxjs/toolkit';
import type { Project } from '$lib/organizations/types';

const projectsAdapter = createEntityAdapter({
	selectId: (project: Project) => project.slug,
	sortComparer: (a: Project, b: Project) => a.slug.localeCompare(b.slug)
});

const projectsSlice = createSlice({
	name: 'projects',
	initialState: projectsAdapter.getInitialState(),
	reducers: {
		addProject: projectsAdapter.addOne,
		addProjects: projectsAdapter.addMany,
		removeProject: projectsAdapter.removeOne,
		removeProjects: projectsAdapter.removeMany,
		upsertProject: projectsAdapter.upsertOne,
		upsertProjects: projectsAdapter.upsertMany
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
