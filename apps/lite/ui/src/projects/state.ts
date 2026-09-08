import { createInitialBranchesState, readStoredBranchFilters } from "#ui/projects/branches.ts";
import {
	createInitialProjectState,
	projectReducers,
	projectSelectors,
	type ProjectState,
} from "#ui/projects/project.ts";
import { createSlice, type PayloadAction } from "@reduxjs/toolkit";

type ProjectSliceState = {
	byProjectId: Record<string, ProjectState>;
};

// Branch filters are the one piece of project state that outlives the session,
// so projects with stored filters start out holding them; every other project
// materializes at the defaults on its first action.
const initialState: ProjectSliceState = {
	byProjectId: Object.fromEntries(
		Object.entries(readStoredBranchFilters()).map(([projectId, filters]) => [
			projectId,
			{ ...createInitialProjectState(), branches: createInitialBranchesState(filters) },
		]),
	),
};

const ensureProjectState = (state: ProjectSliceState, projectId: string): ProjectState => {
	const existingState = state.byProjectId[projectId];
	if (existingState) return existingState;

	const projectState = createInitialProjectState();
	state.byProjectId[projectId] = projectState;
	return projectState;
};

const initialProjectState: ProjectState = createInitialProjectState();

const selectProjectState = (state: ProjectSliceState, projectId: string): ProjectState =>
	state.byProjectId[projectId] ?? initialProjectState;

type AnyProjectReducer = (state: ProjectState, ...args: Array<never>) => void;

type AnyProjectReducerMap = Record<string, AnyProjectReducer>;

type ProjectReducerPayload<T extends AnyProjectReducer> =
	Parameters<T> extends [ProjectState, infer P] ? P & { projectId: string } : { projectId: string };

type FromProjectReducers<T extends AnyProjectReducerMap> = {
	[K in keyof T]: (
		state: ProjectSliceState,
		action: PayloadAction<ProjectReducerPayload<T[K]>>,
	) => void;
};

const fromProjectReducers = <T extends AnyProjectReducerMap>(reducers: T): FromProjectReducers<T> =>
	Object.fromEntries(
		Object.entries(reducers).map(([name, reducer]) => [
			name,
			(state: ProjectSliceState, action: PayloadAction<ProjectReducerPayload<typeof reducer>>) => {
				reducer(ensureProjectState(state, action.payload.projectId), action.payload as never);
			},
		]),
	) as unknown as FromProjectReducers<T>;

type AnyProjectSelectorMap = Record<
	string,
	(state: ProjectState, ...args: Array<never>) => unknown
>;

type FromProjectSelectors<T extends AnyProjectSelectorMap> = {
	[K in keyof T]: T[K] extends (state: ProjectState, ...args: infer A) => infer R
		? (state: ProjectSliceState, projectId: string, ...args: A) => R
		: never;
};

const fromProjectSelectors = <T extends AnyProjectSelectorMap>(
	selectors: T,
): FromProjectSelectors<T> =>
	Object.fromEntries(
		Object.entries(selectors).map(([name, selector]) => [
			name,
			(state: ProjectSliceState, projectId: string, ...args: Array<never>) =>
				selector(selectProjectState(state, projectId), ...args),
		]),
	) as FromProjectSelectors<T>;

export const projectSlice = createSlice({
	name: "project",
	initialState,
	reducers: fromProjectReducers(projectReducers),
	selectors: fromProjectSelectors(projectSelectors),
});
