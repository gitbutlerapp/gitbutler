import type { ForgeName } from '$lib/forge/interface/forge';

export type KeyType = 'gitCredentialsHelper' | 'local' | 'systemExecutable';
export type LocalKey = {
	local: { private_key_path: string };
};

export type AuthKey = Exclude<KeyType, 'local'> | LocalKey;

export type Project = {
	id: string;
	title: string;
	description?: string;
	path: string;
	api?: CloudProject & {
		sync: boolean;
		sync_code: boolean | undefined;
		reviews: boolean | undefined;
	};
	preferred_key: AuthKey;
	ok_with_force_push: boolean;
	force_push_protection: boolean;
	omit_certificate_check: boolean | undefined;
	use_diff_context: boolean | undefined;
	snapshot_lines_threshold: number | undefined;
	// Produced just for the frontend to determine if the project is open in any window.
	is_open: boolean;
	forge_override: ForgeName | undefined;
};

export function vscodePath(path: string) {
	return path.includes('\\') ? '/' + path.replace('\\', '/') : path;
}

export function gitAuthType(preferredKey?: AuthKey): string {
	if (typeof preferredKey === 'object' && preferredKey !== null && 'local' in preferredKey) {
		return 'local';
	}
	return preferredKey as KeyType;
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
