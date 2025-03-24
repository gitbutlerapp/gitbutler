import { parse } from 'comment-json';
import { readFileSync, statSync } from 'node:fs';
import path from 'node:path';

function isRelative(path: string) {
	return path.startsWith('./') || path.startsWith('../');
}

type TsConfig = {
	extends?: string;
	compilerOptions: {
		paths?: Record<string, string[]>;
	};
};

/**
 * Takes a file or directory path and tries to find the nearest tsconfig.json.
 */
function findTsConfigDirectory(absolutePath: string): string | undefined {
	const pathSegments = absolutePath.split('/').slice(1);

	while (pathSegments.length > 0) {
		const configPath = '/' + path.join(...pathSegments, 'tsconfig.json');
		try {
			const configStat = statSync(configPath);
			if (configStat.isFile()) break;
		} catch (_) {
			/* empty */
		}

		pathSegments.pop();
	}
	if (pathSegments.length === 0) return;
	return '/' + path.join(...pathSegments);
}

/**
 * Returns and formats the tsconfig paths that are relevant to the given file.
 *
 * We take the given path and keep looking in the parent directories until a
 * tsconfig is found.
 *
 * Once we have found a tsconfig (and any other super-tsconfigs), we take the
 * list of `paths` if present and globalize them.
 *
 * We choose to absolutize any entries in the `paths` that we find.
 *
 * This is because the destination of a `paths` entry is specified as something
 * relative to the current tsconfig file. If we were to return this it would be
 * very hard to use, so we instead resolve it to an absolute path.
 */
function findTsConfigPaths(absoluteImportPath: string): Record<string, string[]> | undefined {
	const tsConfigDirectory = findTsConfigDirectory(absoluteImportPath);
	if (!tsConfigDirectory) return;

	const configPath = path.join(tsConfigDirectory, 'tsconfig.json');
	const config = parse(readFileSync(configPath).toString()) as any as TsConfig;

	let paths: Record<string, string[]> = {};

	// TS Config files can extend other TS Config files, so we should also
	// consider the paths of the super-configs
	if (config.extends) {
		const otherConfigPath = path.normalize(path.join(tsConfigDirectory, config.extends));
		const otherPaths = findTsConfigPaths(otherConfigPath);
		if (otherPaths) paths = otherPaths;
	}

	if (!config.compilerOptions.paths) return paths;

	// Loop over each entry and absolutize the paths.
	// There may paths that are actually reverring to node modules, but there
	// is no real point in filtering these out as it will just result in
	// something nonsensical and likly to never match.
	for (const [key, relativePaths] of Object.entries(config.compilerOptions.paths)) {
		paths[key] ||= [];
		for (const relativePath of relativePaths) {
			const globalizedPath = path.normalize(path.join(tsConfigDirectory, relativePath));
			paths[key]!.push(globalizedPath);
		}
	}

	return paths;
}

/**
 * Checks whether a path entry ends in a glob.
 *
 * The wildcard * can be used in any part of the key and values, but this is a
 * very rare usage, and so I'm not going to handle it.
 */
function isGlob(path: string) {
	return path.endsWith('/*');
}

function removeGlob(path: string) {
	if (isGlob(path)) {
		return path.slice(0, -1);
	}
	return path;
}

/**
 * Takes a relative import and returns the absolute version of it, if possible.
 */
function formatAsNonRelative(absoluteImportPath: string): string | undefined {
	const paths = findTsConfigPaths(absoluteImportPath);
	if (!paths) return;

	let newPath: string | undefined;
	loop: for (const [key, absolutePaths] of Object.entries(paths)) {
		for (const absolutePath of absolutePaths) {
			// The entries are always either just a file or end with `/*` to
			// indicate a glob (technically it could be in the middle, but I'm
			// not bothered to consider that...)
			//
			// We want to strip off the trailing asterisk if it exists. That
			// way when we go to add add the key (which also has the trailing
			// astrisk removed) we end up with a valid import.
			const deglobbedPath = removeGlob(absolutePath);

			if (absoluteImportPath.startsWith(deglobbedPath)) {
				newPath = absoluteImportPath.replace(deglobbedPath, '');
				newPath = removeGlob(key) + newPath;
				break loop;
			}
		}
	}

	return newPath;
}

function create(context: any) {
	return {
		ImportDeclaration: (node: any) => {
			if (isRelative(node.source.value)) {
				const importPath: string = node.source.value;
				const absoluteImportPath: string = path.join(
					path.dirname(context.getFilename()),
					importPath
				);

				const formattedPath = formatAsNonRelative(absoluteImportPath);

				if (formattedPath) {
					const message = `Import statements should have an absolute path where possible (${formattedPath})`;
					context.report({
						node,
						message,
						fix: (fixer: any) => {
							return fixer.replaceTextRange(
								[node.source.range[0] + 1, node.source.range[1] - 1],
								formattedPath
							);
						}
					});
				}
			}
		}
	};
}

export const noRelativeImportPaths = {
	meta: {
		type: 'layout',
		fixable: 'code',
		schema: {
			type: 'array',
			minItems: 0,
			maxItems: 1,
			items: [
				{
					type: 'object',
					properties: {},
					additionalProperties: false
				}
			]
		}
	},
	create
};
