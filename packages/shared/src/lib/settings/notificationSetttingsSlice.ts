import { buildLoadableTable } from '$lib/redux/defaultSlices';
import { type LoadableNotificationSettings } from '$lib/settings/types';

export const notificationSettingsTable =
	buildLoadableTable<LoadableNotificationSettings>('notificationSettings');
