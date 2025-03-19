import { registerInterest, type InView } from '$lib/interest/registerInterestFunction.svelte';
import { notificationSettingsTable } from '$lib/settings/notificationSetttingsSlice';
import { NOTIFICATION_SETTINGS_KEY, type LoadableNotificationSettings } from '$lib/settings/types';
import type { AppNotificationSettingsState } from '$lib/redux/store.svelte';
import type { NotificationSettingsService } from '$lib/settings/notificationSettingsService';
import type { Reactive } from '$lib/storeUtils';

export function getNotificationSettingsInterest(
	appState: AppNotificationSettingsState,
	notificationSettingsService: NotificationSettingsService,
	inView?: InView
): Reactive<LoadableNotificationSettings | undefined> {
	const notificationSettingsInterest =
		notificationSettingsService.getNotificationSettingsInterest();
	registerInterest(notificationSettingsInterest, inView);

	const notificationSettings = $derived(
		notificationSettingsTable.selectors.selectById(
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
