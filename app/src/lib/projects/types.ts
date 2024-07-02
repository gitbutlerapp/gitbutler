export type KeyType = 'generated' | 'gitCredentialsHelper' | 'local' | 'systemExecutable';
export type LocalKey = {
	local: { private_key_path: string };
};

export type Key = Exclude<KeyType, 'local'> | LocalKey;

export class Project {
	id!: string;
	title!: string;
	description?: string;
	path!: string;
	api?: CloudProject & { sync: boolean };
	preferred_key!: Key;
	ok_with_force_push!: boolean;
	omit_certificate_check: boolean | undefined;
	use_diff_context: boolean | undefined;
	snapshot_lines_threshold!: number | undefined;
	use_new_locking!: boolean;

	get vscodePath() {
		return this.path.includes('\\') ? '/' + this.path.replace('\\', '/') : this.path;
	}
}

export type CloudProject = {
	name: string;
	description: string | null;
	repository_id: string;
	git_url: string;
	created_at: string;
	updated_at: string;
};
