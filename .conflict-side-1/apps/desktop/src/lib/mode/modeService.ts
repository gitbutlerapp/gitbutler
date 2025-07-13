import { invoke, listen } from '$lib/backend/ipc';
import { RemoteFile } from '$lib/files/file';
import { plainToInstance } from 'class-transformer';
import { derived, writable } from 'svelte/store';
import type { ConflictEntryPresence } from '$lib/conflictEntryPresence';
import type { StackService } from '$lib/stacks/stackService.svelte';

export interface EditModeMetadata {
	commitOid: string;
	branchReference: string;
}

export interface OutsideWorkspaceMetadata {
	/** The name of the currently checked out branch or null if in detached head state. */
	branchName: string | null;
	/** The paths of any files that would conflict with the workspace as it currently is */
	worktreeConflicts: string[];
}

type Mode =
	| { type: 'OpenWorkspace' }
	| {
			type: 'OutsideWorkspace';
			subject: OutsideWorkspaceMetadata;
	  }
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

	constructor(
		private projectId: string,
		private readonly stackService: StackService
	) {}

	private async refresh() {
		const head = await invoke<string>('git_head', { projectId: this.projectId });
		const operatingMode = await invoke<Mode>('operating_mode', { projectId: this.projectId });

		this.headAndMode.set({ head, operatingMode });
	}

	async enterEditMode(commitId: string, stackId: string) {
		this.stackService.enterEditMode({
			projectId: this.projectId,
			commitId,
			stackId
		});
		await this.awaitMode('Edit');
	}

	async abortEditAndReturnToWorkspace() {
		await this.stackService.abortEditAndReturnToWorkspace({
			projectId: this.projectId
		});
		await this.awaitMode('OpenWorkspace');
	}

	async saveEditAndReturnToWorkspace() {
		await this.stackService.saveEditAndReturnToWorkspace({
			projectId: this.projectId
		});
		await this.awaitMode('OpenWorkspace');
	}

	async getInitialIndexState() {
		const rawOutput = await invoke<unknown[][]>('edit_initial_index_state', {
			projectId: this.projectId
		});

		return rawOutput.map((entry) => {
			return [plainToInstance(RemoteFile, entry[0]), entry[1] as ConflictEntryPresence | undefined];
		}) as [RemoteFile, ConflictEntryPresence | undefined][];
	}

	async awaitMode(mode: Mode['type']): Promise<void> {
		return await new Promise((resolve) => {
			const unsubscribe = this.mode.subscribe((operatingMode) => {
				if (operatingMode && operatingMode?.type === mode) {
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
