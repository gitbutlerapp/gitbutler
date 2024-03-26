import { invoke, listen as listenIpc } from '$lib/backend/ipc';
import { unsubscribe } from '$lib/utils/unsubscribe';
import { goto } from '$app/navigation';

export class MenuBarController {
	static instance?: MenuBarController;

	static getInstance() {
		if (!this.instance) {
			this.instance = new MenuBarController();
		}

		return this.instance;
	}

	private subscription?: () => Promise<void>;

	async setProjectId(projectId: string | undefined) {
		// Ensure old subscription is removed before creating a new one
		await unsubscribe(this.subscription)();

		// If no project id is provided, stay unsubscribed
		if (!projectId) return;

		invoke('menu_item_set_enabled', {
			menuItemId: 'project/settings',
			enabled: true
		});

		const projectSettingsSubscription = listenIpc<string>('menu://project/settings/clicked', () => {
			goto(`/${projectId}/settings/`);
		});

		this.subscription = async () => {
			await unsubscribe(projectSettingsSubscription)();
			await invoke('menu_item_set_enabled', {
				menuItemId: 'project/settings',
				enabled: false
			});
		};
	}
}
