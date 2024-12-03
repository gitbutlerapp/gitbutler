import { CommandService } from '$lib/backend/ipc';
import { derived, get, type Readable } from 'svelte/store';
import type { Project } from '$lib/backend/projects';
import type { User } from '$lib/stores/user';

export class SyncedSnapshotService {
	/**
	 * Signal for the frontend to choose whether or not to provide the
	 * takeSyncedSnapshot button.
	 */
	canTakeSnapshot: Readable<boolean>;

	#joinedUserAndProject: Readable<{
		user: User | undefined;
		project: Project | undefined;
	}>;

	constructor(
		private readonly commandService: CommandService,
		private readonly user: Readable<User | undefined>,
		private readonly project: Readable<Project | undefined>
	) {
		this.#joinedUserAndProject = derived([this.user, this.project], ([user, project]) => {
			return { user, project };
		});

		this.canTakeSnapshot = derived(this.#joinedUserAndProject, ({ user, project }) => {
			return this.canTakeSnapshotGivenUserAndProject(user, project);
		});
	}

	async takeSyncedSnapshot(stackId?: string): Promise<string> {
		// Take a snapshot
		const { user, project } = get(this.#joinedUserAndProject);

		// Project and user are now defined
		if (!this.canTakeSnapshotGivenUserAndProject(user, project)) {
			throw new Error('Cannot take a snapshot');
		}

		const snapshotOid = await this.commandService.invoke<string>('take_synced_snapshot', {
			projectId: project!.id,
			user: user!,
			stackId
		});

		return snapshotOid;
	}

	private canTakeSnapshotGivenUserAndProject(user: User | undefined, project: Project | undefined) {
		return user !== undefined && !!project?.api?.sync;
	}
}
