import { listen } from '$lib/backend/ipc';
import { goto } from '$app/navigation';

export function handleMenuActions(projectId: string) {
	return listen<string>(`menuAction`, (event) => {
		if (event.payload == 'projectSettings') {
			goto(`/${projectId}/settings`);
		}
	});
}
