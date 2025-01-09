import { loadableUpsert, loadableUpsertMany } from '$lib/network/loadable';
import { createEntityAdapter, createSlice } from '@reduxjs/toolkit';
import type { LoadableUser } from '$lib/users/types';

const usersAdapter = createEntityAdapter<LoadableUser, LoadableUser['id']>({
	selectId: (user: LoadableUser) => user.id
});

const usersSlice = createSlice({
	name: 'users',
	initialState: usersAdapter.getInitialState(),
	reducers: {
		addUser: usersAdapter.addOne,
		addUsers: usersAdapter.addMany,
		removeUser: usersAdapter.removeOne,
		removeUsers: usersAdapter.removeMany,
		upsertUser: loadableUpsert(usersAdapter),
		upsertUsers: loadableUpsertMany(usersAdapter)
	}
});

export const usersReducer = usersSlice.reducer;

export const usersSelectors = usersAdapter.getSelectors();
export const { addUser, addUsers, removeUser, removeUsers, upsertUser, upsertUsers } =
	usersSlice.actions;
