import { type LoadableNotificationSettings } from './types';
import { createEntityAdapter, createSlice } from '@reduxjs/toolkit';

const notificationSettingsAdapter = createEntityAdapter<
	LoadableNotificationSettings,
	LoadableNotificationSettings['id']
>({
	selectId: (notificationSettings) => notificationSettings.id
});

const notificationSettingsSlice = createSlice({
	name: 'notificationSettings',
	initialState: notificationSettingsAdapter.getInitialState(),
	reducers: {
		upsertNotificationSettings: notificationSettingsAdapter.upsertOne,
		upsertNotificationSettingsMany: notificationSettingsAdapter.upsertMany
	}
});

export const notificationSettingsReducer = notificationSettingsSlice.reducer;

export const notificationSettingsSelectors = notificationSettingsAdapter.getSelectors();
export const { upsertNotificationSettings, upsertNotificationSettingsMany } =
	notificationSettingsSlice.actions;
