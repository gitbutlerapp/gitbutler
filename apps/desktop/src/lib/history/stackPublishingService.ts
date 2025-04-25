import { CommandService } from '$lib/backend/ipc';
import { derived, get, type Readable } from 'svelte/store';
import type { Project } from '$lib/project/project';
import type { User } from '$lib/user/user';

export class StackPublishingService {
	/**
	 * Signal for the frontend to choose whether or not to provide the
	 * takeSyncedSnapshot button.
	 */
	canPublish: Readable<boolean>;

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

		this.canPublish = derived(this.#joinedUserAndProject, ({ user, project }) => {
			return this.canPublishStack(user, project);
		});
	}

	// Returns the reviewID of the pushed branch...
	async upsertStack(stackId: string, topBranch: string): Promise<string> {
		// Take a snapshot
		const { user, project } = get(this.#joinedUserAndProject);

		// Project and user are now defined
		if (!this.canPublishStack(user, project)) {
			throw new Error('Cannot publish branch');
		}

		return await this.commandService.invoke<string>('push_stack_to_review', {
			projectId: project!.id,
			user: user!,
			stackId,
			topBranch
		});
	}

	private canPublishStack(user: User | undefined, project: Project | undefined) {
		return user !== undefined && !!project?.api && !!project?.api.reviews;
	}
}
