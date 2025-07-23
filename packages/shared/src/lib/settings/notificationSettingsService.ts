import { InterestStore, type Interest } from '$lib/interest/interestStore';
import { errorToLoadable } from '$lib/network/loadable';
import { POLLING_GLACIALLY } from '$lib/polling';
import { notificationSettingsTable } from '$lib/settings/notificationSetttingsSlice';
import {
	apiToNotificationSettings,
	NOTIFICATION_SETTINGS_KEY,
	notificationSettingsToApi,
	type ApiNotificationSettings,
	type LoadableNotificationSettings,
	type PatchNotificationSettingsParams
} from '$lib/settings/types';
import type { HttpClient } from '$lib/network/httpClient';
import type { AppDispatch } from '$lib/redux/store.svelte';
import { InjectionToken } from '$lib/context';

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
				this.appDispatch.dispatch(
					notificationSettingsTable.addOne({ status: 'loading', id: NOTIFICATION_SETTINGS_KEY })
				);
				try {
					const apiNotificationSettings =
						await this.httpClient.get<ApiNotificationSettings>('settings/notifications');

					const notificationSettings: LoadableNotificationSettings = {
						status: 'found',
						id: NOTIFICATION_SETTINGS_KEY,
						value: apiToNotificationSettings(apiNotificationSettings)
					};

					this.appDispatch.dispatch(notificationSettingsTable.upsertOne(notificationSettings));
				} catch (error: unknown) {
					this.appDispatch.dispatch(
						notificationSettingsTable.addOne(errorToLoadable(error, NOTIFICATION_SETTINGS_KEY))
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

export const NOTIFICATION_SETTINGS_SERVICE_TOKEN = new InjectionToken<NotificationSettingsService>('NotificationSettingsService');
