import { invoke, listen } from '$lib/backend/ipc';
import { RemoteFile } from '$lib/files/file';
import { plainToInstance } from 'class-transformer';
import { derived, writable } from 'svelte/store';
import type { ConflictEntryPresence } from '$lib/conflictEntryPresence';

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

	async enterEditMode(commitOid: string, stackId: string) {
		await invoke('enter_edit_mode', {
			projectId: this.projectId,
			commitOid,
			stackId
		});
	}

	async abortEditAndReturnToWorkspace() {
		await invoke('abort_edit_and_return_to_workspace', {
			projectId: this.projectId
		});
	}

	async saveEditAndReturnToWorkspace() {
		await invoke('save_edit_and_return_to_workspace', {
			projectId: this.projectId
		});
	}

	async getInitialIndexState() {
		const rawOutput = await invoke<unknown[][]>('edit_initial_index_state', {
			projectId: this.projectId
		});

		return rawOutput.map((entry) => {
			return [plainToInstance(RemoteFile, entry[0]), entry[1] as ConflictEntryPresence | undefined];
		}) as [RemoteFile, ConflictEntryPresence | undefined][];
	}

	async awaitNotEditing(): Promise<void> {
		return await new Promise((resolve) => {
			const unsubscribe = this.mode.subscribe((operatingMode) => {
				if (operatingMode && operatingMode?.type !== 'Edit') {
					resolve();

					setTimeout(() => {
						unsubscribe();
					}, 0);
				}
			});
		});
	}
}

function subscribeToHead(projectId: string, callback: (headAndMode: HeadAndMode) => void) {
	return listen<HeadAndMode>(`project://${projectId}/git/head`, (event) => callback(event.payload));
}
