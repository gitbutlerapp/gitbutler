const KNOWN_LOCKFILE_NAMES = [
	'package-lock.json',
	'yarn.lock',
	'pnpm-lock.yaml',
	'bun.lockb',
	'poetry.lock',
	'Pipfile.lock',
	'Gemfile.lock',
	'composer.lock',
	'build.gradle.lockfile',
	'gradle.lockfile',
	'packages.lock.json',
	'project.assets.json',
	'go.sum',
	'Cargo.lock',
	'pubspec.lock',
	'Package.resolved',
	'mix.lock',
	'cabal.project.freeze',
	'stack.yaml.lock',
	'rebar.lock',
	'opam.locked',
	'cpanfile.snapshot',
	'renv.lock',
	'shard.lock',
	'Manifest.toml',
	'nimble.lock'
];

function isFileName(filePath: string, fileName: string): boolean {
	if (filePath === fileName) {
		return true;
	}
	const separators = ['\\', '/'];
	for (const separator of separators) {
		if (filePath.endsWith(`${separator}${fileName}`)) {
			return true;
		}
	}
	return false;
}

export function isLockfile(filePath: string): boolean {
	return KNOWN_LOCKFILE_NAMES.some((known) => isFileName(filePath, known));
}
