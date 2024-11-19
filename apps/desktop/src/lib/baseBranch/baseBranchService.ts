import { BaseBranch, NoDefaultTarget } from './baseBranch';
import { Code, invoke } from '$lib/backend/ipc';
import { showError } from '$lib/notifications/toasts';
import { uniqueDerived } from '$lib/stores/uniqueStore';
import { parseRemoteUrl } from '$lib/url/gitUrl';
import { plainToInstance } from 'class-transformer';
import { derived, writable } from 'svelte/store';

export interface RemoteBranchInfo {
	name: string;
}

export class BaseBranchService {
	// Primary private writable that this class emits to.
	private readonly _base = writable<BaseBranch | null | undefined>(undefined, () => {
		this.refresh();
	});

	// Deduplciated since updates are frequent.
	readonly base = uniqueDerived(this._base);

	// Deduplicated repo information.
	readonly repo = uniqueDerived(
		derived(this.base, (base) => {
			return base ? parseRemoteUrl(base.remoteUrl) : undefined;
		})
	);

	// Deduplicated push repo information.
	readonly pushRepo = uniqueDerived(
		derived(this.base, (base) => {
			return base ? parseRemoteUrl(base.pushRemoteUrl) : undefined;
		})
	);

	readonly loading = writable(false);
	readonly error = writable();

	constructor(private readonly projectId: string) {}

	async refresh(): Promise<void> {
		this.loading.set(true);
		try {
			const baseBranch = plainToInstance(
				BaseBranch,
				await invoke<any>('get_base_branch_data', { projectId: this.projectId })
			);
			if (!baseBranch) this.error.set(new NoDefaultTarget());
			this._base.set(baseBranch);
		} catch (err: any) {
			this.error.set(err);
			throw err;
		} finally {
			this.loading.set(false);
		}
	}

	async fetchFromRemotes(action: string | undefined = undefined) {
		this.loading.set(true);
		try {
			// Note that we expect the back end to emit new fetches event, and therefore
			// trigger a base branch reload. It feels a bit awkward and should be improved.
			await invoke<void>('fetch_from_remotes', {
				projectId: this.projectId,
				action: action || 'auto'
			});
		} catch (err: any) {
			if (err.code === Code.DefaultTargetNotFound) {
				// Swallow this error since user should be taken to project setup page
				return;
			} else if (err.code === Code.ProjectsGitAuth) {
				showError('Failed to authenticate', err);
			} else if (action !== undefined) {
				showError('Failed to fetch', err);
			}
			console.error(err);
		} finally {
			this.loading.set(false);
		}
	}

	async setTarget(branch: string, pushRemote: string | undefined = undefined) {
		this.loading.set(true);
		await invoke<BaseBranch>('set_base_branch', {
			projectId: this.projectId,
			branch,
			pushRemote
		});
		await this.fetchFromRemotes();
	}

	async push(withForce?: boolean) {
		this.loading.set(true);
		try {
			await invoke<void>('push_base_branch', {
				projectId: this.projectId,
				withForce
			});
		} catch (err: any) {
			if (err.code === Code.DefaultTargetNotFound) {
				// Swallow this error since user should be taken to project setup page
				return;
			} else if (err.code === Code.ProjectsGitAuth) {
				showError('Failed to authenticate', err);
			} else {
				showError('Failed to push', err);
			}
			console.error(err);
		}
		await this.fetchFromRemotes();
	}
}

export async function getRemoteBranches(
	projectId: string | undefined
): Promise<RemoteBranchInfo[]> {
	if (!projectId) return [];
	return await invoke<Array<string>>('git_remote_branches', { projectId }).then((branches) =>
		branches
			.map((name) => name.substring(13))
			.sort((a, b) => a.localeCompare(b))
			.map((name) => ({ name }))
	);
}
