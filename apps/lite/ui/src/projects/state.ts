import {
	createInitialProjectState,
	projectReducers,
	projectSelectors,
	type ProjectState,
} from "#ui/projects/project.ts";
import { createSlice, type PayloadAction } from "@reduxjs/toolkit";

type ProjectStateTable = Record<string, ProjectState>;

const initialState: ProjectStateTable = {};

const ensureProjectState = (state: ProjectStateTable, projectId: string): ProjectState => {
	const existingState = state[projectId];
	if (existingState) return existingState;

	const projectState = createInitialProjectState();
	state[projectId] = projectState;
	return projectState;
};

const initialProjectState: ProjectState = createInitialProjectState();

const selectProjectState = (state: ProjectStateTable, projectId: string): ProjectState =>
	state[projectId] ?? initialProjectState;

type AnyProjectReducer = (state: ProjectState, ...args: Array<never>) => void;

type AnyProjectReducerMap = Record<string, AnyProjectReducer>;

type ProjectReducerPayload<T extends AnyProjectReducer> =
	Parameters<T> extends [ProjectState, infer P] ? P & { projectId: string } : { projectId: string };

type ProjectSliceReducers<T extends AnyProjectReducerMap> = {
	[K in keyof T]: (
		state: ProjectStateTable,
		action: PayloadAction<ProjectReducerPayload<T[K]>>,
	) => void;
};

const fromProjectReducers = <T extends AnyProjectReducerMap>(
	reducers: T,
): ProjectSliceReducers<T> =>
	Object.fromEntries(
		Object.entries(reducers).map(([name, reducer]) => [
			name,
			(state: ProjectStateTable, action: PayloadAction<ProjectReducerPayload<typeof reducer>>) => {
				reducer(ensureProjectState(state, action.payload.projectId), action.payload as never);
			},
		]),
	) as unknown as ProjectSliceReducers<T>;

type AnyProjectSelectorMap = Record<
	string,
	(state: ProjectState, ...args: Array<never>) => unknown
>;

type ProjectSliceSelectors<T extends AnyProjectSelectorMap> = {
	[K in keyof T]: T[K] extends (state: ProjectState, ...args: infer A) => infer R
		? (state: ProjectStateTable, projectId: string, ...args: A) => R
		: never;
};

const fromProjectSelectors = <T extends AnyProjectSelectorMap>(
	selectors: T,
): ProjectSliceSelectors<T> =>
	Object.fromEntries(
		Object.entries(selectors).map(([name, selector]) => [
			name,
			(state: ProjectStateTable, projectId: string, ...args: Array<never>) =>
				selector(selectProjectState(state, projectId), ...args),
		]),
	) as ProjectSliceSelectors<T>;

export const projectSlice = createSlice({
	name: "project",
	initialState,
	reducers: fromProjectReducers(projectReducers),
	selectors: fromProjectSelectors(projectSelectors),
});
