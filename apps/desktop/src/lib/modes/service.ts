import { invoke, listen } from '$lib/backend/ipc';
import { derived, writable } from 'svelte/store';

export interface EditModeMetadata {
	commitOid: string;
	branchReference: string;
}

type Mode =
	| { type: 'OpenWorkspace' }
	| { type: 'OutsideWorkspace' }
	| {
			type: 'Edit';
			subject: EditModeMetadata;
	  };
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

	async enterEditMode(commitOid: string, branchReference: string) {
		await invoke('enter_edit_mode', {
			projectId: this.projectId,
			commitOid,
			branchReference
		});
	}

	async saveEditAndReturnToWorkspace() {
		await invoke('save_edit_and_return_to_workspace', {
			projectId: this.projectId
		});
	}
}

function subscribeToHead(projectId: string, callback: (headAndMode: HeadAndMode) => void) {
	return listen<HeadAndMode>(`project://${projectId}/git/head`, (event) => callback(event.payload));
}
