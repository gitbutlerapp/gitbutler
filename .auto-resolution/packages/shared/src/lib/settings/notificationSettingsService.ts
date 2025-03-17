import { upsertNotificationSettings } from './notificationSetttingsSlice';
import {
	apiToNotificationSettings,
	NOTIFICATION_SETTINGS_KEY,
	notificationSettingsToApi,
	type ApiNotificationSettings,
	type LoadableNotificationSettings,
	type PatchNotificationSettingsParams
} from './types';
import { InterestStore, type Interest } from '$lib/interest/interestStore';
import { errorToLoadable } from '$lib/network/loadable';
import { POLLING_GLACIALLY } from '$lib/polling';
import type { HttpClient } from '$lib/network/httpClient';
import type { AppDispatch } from '$lib/redux/store.svelte';

export class NotificationSettingsService {
	private readonly notificationSettingsInterest = new InterestStore<{
		key: typeof NOTIFICATION_SETTINGS_KEY;
	}>(POLLING_GLACIALLY);

	constructor(
		private readonly httpClient: HttpClient,
		private readonly appDispatch: AppDispatch
	) {}

	getNotificationSettingsInterest(): Interest {
		return this.notificationSettingsInterest
			.findOrCreateSubscribable({ key: NOTIFICATION_SETTINGS_KEY }, async () => {
				try {
					const apiNotificationSettings =
						await this.httpClient.get<ApiNotificationSettings>('settings/notifications');

					const notificationSettings: LoadableNotificationSettings = {
						status: 'found',
						id: NOTIFICATION_SETTINGS_KEY,
						value: apiToNotificationSettings(apiNotificationSettings)
					};

					this.appDispatch.dispatch(upsertNotificationSettings(notificationSettings));
				} catch (error: unknown) {
					this.appDispatch.dispatch(
						upsertNotificationSettings(errorToLoadable(error, NOTIFICATION_SETTINGS_KEY))
					);
				}
			})
			.createInterest();
	}

	async refresh() {
		await this.notificationSettingsInterest.invalidate({ key: NOTIFICATION_SETTINGS_KEY });
	}

	async updateNotificationSettings(params: PatchNotificationSettingsParams) {
		await this.httpClient.patch('settings/notifications', {
			body: notificationSettingsToApi(params)
		});
		await this.refresh();
	}
}
