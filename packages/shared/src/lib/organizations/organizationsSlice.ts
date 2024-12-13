import { createEntityAdapter, createSlice } from '@reduxjs/toolkit';
import type { LoadableOrganization } from '$lib/organizations/types';

const organizationsAdapter = createEntityAdapter<LoadableOrganization, LoadableOrganization['id']>({
	selectId: (organization: LoadableOrganization) => organization.id
});

const organizationsSlice = createSlice({
	name: 'organizations',
	initialState: organizationsAdapter.getInitialState(),
	reducers: {
		addOrganization: organizationsAdapter.addOne,
		addOrganizations: organizationsAdapter.addMany,
		removeOrganization: organizationsAdapter.removeOne,
		removeOrganizations: organizationsAdapter.removeMany,
		upsertOrganization: organizationsAdapter.upsertOne,
		upsertOrganizations: organizationsAdapter.upsertMany
	}
});

export const organizationsReducer = organizationsSlice.reducer;

export const organizationsSelectors = organizationsAdapter.getSelectors();
export const {
	addOrganization,
	addOrganizations,
	removeOrganization,
	removeOrganizations,
	upsertOrganization,
	upsertOrganizations
} = organizationsSlice.actions;
