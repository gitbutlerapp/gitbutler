import { loadableUpsert, loadableUpsertMany } from '$lib/network/loadable';
import { type LoadableUserIdByLogin, type LoadableUser } from '$lib/users/types';
import { createEntityAdapter, createSlice } from '@reduxjs/toolkit';

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

const usersByLoginAdapter = createEntityAdapter<LoadableUserIdByLogin, LoadableUserIdByLogin['id']>(
	{
		selectId: (user: LoadableUserIdByLogin) => user.id
	}
);

const usersByLoginSlice = createSlice({
	name: 'usersByLogin',
	initialState: usersByLoginAdapter.getInitialState(),
	reducers: {
		addUserByLogin: usersByLoginAdapter.addOne,
		addUsersByLogin: usersByLoginAdapter.addMany,
		removeUserByLogin: usersByLoginAdapter.removeOne,
		removeUsersByLogin: usersByLoginAdapter.removeMany,
		upsertUserByLogin: loadableUpsert(usersByLoginAdapter),
		upsertUsersByLogin: loadableUpsertMany(usersByLoginAdapter)
	}
});

export const usersByLoginReducer = usersByLoginSlice.reducer;

export const usersByLoginSelectors = usersByLoginAdapter.getSelectors();
export const {
	addUserByLogin,
	addUsersByLogin,
	removeUserByLogin,
	removeUsersByLogin,
	upsertUserByLogin,
	upsertUsersByLogin
} = usersByLoginSlice.actions;
