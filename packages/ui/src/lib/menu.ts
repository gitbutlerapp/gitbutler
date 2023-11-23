import { emit } from '$lib/utils/events';
import { invoke, listen as listenIpc } from '$lib/backend/ipc';

export function subscribe(projectId: string) {
	invoke('menu_item_set_enabled', {
		menuItemId: 'project/settings',
		enabled: true
	});
	const unsubscribeProjectSettings = listenIpc<string>('menu://project/settings/clicked', () => {
		emit('goto', `/${projectId}/settings/`);
	});
	return () => {
		unsubscribeProjectSettings();
		invoke('menu_item_set_enabled', {
			menuItemId: 'project/settings',
			enabled: false
		});
	};
}
