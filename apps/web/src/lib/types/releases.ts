export interface Build {
	os: 'windows' | 'darwin' | 'linux';
	arch: 'x86_64' | 'aarch64';
	url: string;
	file: string;
	platform: string;
}

export interface Release {
	version: string;
	notes: string;
	sha: string;
	channel: 'release' | string;
	build_version: string;
	released_at: string;
	builds: Build[];
}
