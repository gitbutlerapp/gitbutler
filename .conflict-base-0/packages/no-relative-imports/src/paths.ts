import path from 'node:path';

/**
 * Checks whether a path entry ends in a glob.
 *
 * The wildcard * can be used in any part of the key and values, but this is a
 * very rare usage, and so I'm not going to handle it.
 */
function hasWildcard(path: string) {
	return path.endsWith('*');
}

function isHandledGlob(path: string): boolean {
	const asteriskOccurences = path.split('*').length - 1;
	// If there is no globbing, we can handle it
	if (asteriskOccurences === 0) return true;
	// If there is more than one glob, we can't handle it
	if (asteriskOccurences > 1) return false;

	return hasWildcard(path);
}

function removeGlob(path: string) {
	if (hasWildcard(path)) {
		return path.slice(0, -1);
	}
	return path;
}

/**
 * Represents an entry in the paths config.
 */
export class PathEntry {
	private readonly absoluteTarget: string;

	constructor(
		configDirectory: string,
		private readonly key: string,
		target: string
	) {
		// The target is defined as being relative to the tsconfig's directory.
		// When it comes to matching, it's easier to work on the absolute
		// version instead.
		this.absoluteTarget = path.normalize(path.join(configDirectory, target));
	}

	tryAliasImport(absoluteImportPath: string): string | undefined {
		if (hasWildcard(this.absoluteTarget)) {
			// If the target is a glob, we want to see if the import
			// starts with the target path. If it does, we then want
			// to replace the portion that matches the glob
			// and replace it with the key.
			const deglobbedPath = removeGlob(this.absoluteTarget);

			if (absoluteImportPath.startsWith(deglobbedPath)) {
				if (hasWildcard(this.key)) {
					let newPath = absoluteImportPath.replace(deglobbedPath, '');
					newPath = removeGlob(this.key) + newPath;
					return newPath;
				} else {
					return this.key;
				}
			}
		} else {
			// If the target is not a glob, then we can directly
			// replace the import with the alias.
			if (absoluteImportPath === this.absoluteTarget) {
				return this.key;
			}
		}
	}
}

/**
 * Represents the path entries from a TSConfig path
 */
export class ConfigPaths {
	/**
	 * A map with the alias name as the key, and the corresponding paths values
	 * in an array.
	 *
	 * When we make use of this object in `tryAliasImport` we don't make use of
	 * the keys, but they are useful for defining the overriding behaviour
	 * correctly.
	 */
	private readonly paths: Record<string, PathEntry[]> = {};

	/**
	 * Pushes a list of paths declarations for matching on later. If a key gets
	 * pushed a second time, it will override the previous entry.
	 *
	 * As such, it is expected that this function is first called with the base
	 * most config followed by the super-most configs
	 *
	 * There is no specification for how paths should be merged between
	 * extended configs, so I assume the intent of the user if a key is
	 * provided twice, is to simply replace the other entry.
	 *
	 * @argument configDirectory The directory that contains the tsconfig/jsconfig
	 * @argument paths The paths property of the tsconfig/jsconfig
	 */
	pushPaths(configDirectory: string, paths: Record<string, string[]>): void {
		for (const [key, relativePaths] of Object.entries(paths)) {
			if (!isHandledGlob(key)) continue;

			// If we have seen the
			this.paths[key] = [];

			for (const target of relativePaths) {
				if (!isHandledGlob(target)) continue;
				const entry = new PathEntry(configDirectory, key, target);
				this.paths[key]!.push(entry);
			}
		}
	}

	/**
	 * Takes a path that is being imported that has already be made absolute
	 * and tries to match against an alias. If it matches against an alias it
	 * will return the aliased import
	 */
	tryAliasImport(absoluteImportPath: string): string | undefined {
		for (const entry of Object.values(this.paths).flat()) {
			const result = entry.tryAliasImport(absoluteImportPath);
			if (result) return result;
		}
	}

	get empty() {
		return Object.values(this.paths).length === 0;
	}
}
