import type { ForgeName } from '$lib/forge/interface/forge';

export type KeyType = 'gitCredentialsHelper' | 'local' | 'systemExecutable';
export type LocalKey = {
	local: { private_key_path: string };
};

export type Key = Exclude<KeyType, 'local'> | LocalKey;

export class Project {
	id!: string;
	title!: string;
	description?: string;
	path!: string;
	api?: CloudProject & {
		sync: boolean;
		sync_code: boolean | undefined;
		reviews: boolean | undefined;
	};
	preferred_key!: Key;
	ok_with_force_push!: boolean;
	omit_certificate_check: boolean | undefined;
	use_diff_context: boolean | undefined;
	snapshot_lines_threshold!: number | undefined;
	// Produced just for the frontend to determine if the project is open in any window.
	is_open!: boolean;
	forge_override!: ForgeName | undefined;

	get vscodePath() {
		return this.path.includes('\\') ? '/' + this.path.replace('\\', '/') : this.path;
	}

	gitAuthType(): string {
		if (
			typeof this.preferred_key === 'object' &&
			this.preferred_key !== null &&
			'local' in this.preferred_key
		) {
			return 'local';
		}
		return this.preferred_key as KeyType;
	}
}

export type CloudProject = {
	name: string;
	description: string | null;
	repository_id: string;
	git_url: string;
	code_git_url: string;
	created_at: string;
	updated_at: string;
};
