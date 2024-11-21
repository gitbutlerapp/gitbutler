import { selectSelf } from '$lib/redux/selectSelf';
import { createSelector, createSlice } from '@reduxjs/toolkit';

export interface ExampleState {
	value: number;
}

const initialState: ExampleState = {
	value: 0
};

const exampleSlice = createSlice({
	name: 'example',
	initialState,
	reducers: {
		increment: (state) => {
			state.value += 1;
		},
		decrement: (state) => {
			state.value -= 1;
		}
	}
});

export const { increment, decrement } = exampleSlice.actions;
export const exampleReducer = exampleSlice.reducer;

export const selectExample = createSelector([selectSelf], (state) => state.example);
export const selectExampleValue = createSelector([selectExample], (example) => example.value);
export const selectExampleValueGreaterThan = createSelector(
	[selectExampleValue, (_state: unknown, target: number) => target],
	(value, target: number) => value > target
);
