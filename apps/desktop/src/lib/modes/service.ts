import { invoke, listen } from '$lib/backend/ipc';
import { derived, writable } from 'svelte/store';

type Mode = { type: 'OpenWorkspace' } | { type: 'OutsideWorksapce' } | { type: 'Edit' };
interface HeadAndMode {
	head?: string;
	operatingMode?: Mode;
}

export class ModeService {
	private headAndMode = writable<HeadAndMode>({}, (set) => {
		this.refresh();

		const unsubscribe = subscribeToHead(this.projectId, (headAndMode) => {
			set(headAndMode);
		});

		return unsubscribe;
	});

	readonly head = derived(this.headAndMode, ({ head }) => head);
	readonly mode = derived(this.headAndMode, ({ operatingMode }) => operatingMode);

	constructor(private projectId: string) {}

	private async refresh() {
		const head = await invoke<string>('git_head', { projectId: this.projectId });
		const operatingMode = await invoke<Mode>('operating_mode', { projectId: this.projectId });

		this.headAndMode.set({ head, operatingMode });
	}
}

function subscribeToHead(projectId: string, callback: (headAndMode: HeadAndMode) => void) {
	return listen<HeadAndMode>(`project://${projectId}/git/head`, (event) => callback(event.payload));
}
