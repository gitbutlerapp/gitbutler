import { notificationSettingsSelectors } from './notificationSetttingsSlice';
import { NOTIFICATION_SETTINGS_KEY, type LoadableNotificationSettings } from './types';
import { registerInterest, type InView } from '$lib/interest/registerInterestFunction.svelte';
import type { AppNotificationSettingsState } from '$lib/redux/store.svelte';
import type { Reactive } from '$lib/storeUtils';
import type { NotificationSettingsService } from './notificationSettingsService';

export function getNotificationSettingsInterest(
	appState: AppNotificationSettingsState,
	notificationSettingsService: NotificationSettingsService,
	inView?: InView
): Reactive<LoadableNotificationSettings | undefined> {
	const notificationSettingsInterest =
		notificationSettingsService.getNotificationSettingsInterest();
	registerInterest(notificationSettingsInterest, inView);

	const notificationSettings = $derived(
		notificationSettingsSelectors.selectById(
			appState.notificationSettings,
			NOTIFICATION_SETTINGS_KEY
		)
	);

	return {
		get current() {
			return notificationSettings;
		}
	};
}
