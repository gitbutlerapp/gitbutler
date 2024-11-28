import { createEntityAdapter, createSlice } from '@reduxjs/toolkit';
import type { User } from '$lib/users/types';

const usersAdapter = createEntityAdapter({
	selectId: (user: User) => user.login,
	sortComparer: (a: User, b: User) => a.login.localeCompare(b.login)
});

const usersSlice = createSlice({
	name: 'users',
	initialState: usersAdapter.getInitialState(),
	reducers: {
		addUser: usersAdapter.addOne,
		addUsers: usersAdapter.addMany,
		removeUser: usersAdapter.removeOne,
		removeUsers: usersAdapter.removeMany,
		upsertUser: usersAdapter.upsertOne,
		upsertUsers: usersAdapter.upsertMany
	}
});

export const usersReducer = usersSlice.reducer;

export const usersSelectors = usersAdapter.getSelectors();
export const { addUser, addUsers, removeUser, removeUsers, upsertUser, upsertUsers } =
	usersSlice.actions;
