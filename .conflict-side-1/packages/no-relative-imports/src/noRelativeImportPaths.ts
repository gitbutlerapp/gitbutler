import { ConfigPaths } from './paths.js';
import { parse } from 'comment-json';
import { readFileSync, statSync } from 'node:fs';
import path from 'node:path';

/**
 * Do a basic check to determine if an import is relative
 */
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
 * Returns a list of tsconfigs ordered from the base config to the super
 * config
 */
function findConfigs(absolutePath: string): [string, TsConfig][] {
	const configs: [string, TsConfig][] = [];

	let currentPath = absolutePath;

	while (true) {
		const configDirectory = findTsConfigDirectory(currentPath);
		if (!configDirectory) break;

		const configPath = path.join(configDirectory, 'tsconfig.json');
		const config = parse(readFileSync(configPath).toString()) as any as TsConfig;

		// We are traversing the list of configs from super-most to base-most.
		// Given that we want the output to have the base-most first, we want
		// to unshift the entries into the output array.
		configs.unshift([configDirectory, config]);

		if (config.extends) {
			// The `.json` at the end of the `extends` relative path is
			// optional, so we should add back.
			let extendsPath = config.extends;
			if (!extendsPath.endsWith('.json')) {
				extendsPath = `${extendsPath}.json`;
			}
			currentPath = path.join(configDirectory, extendsPath);
		} else {
			break;
		}
	}

	return configs;
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
function findTsConfigPaths(absoluteImportPath: string): ConfigPaths {
	const configPaths = new ConfigPaths();

	const configs = findConfigs(absoluteImportPath);

	for (const [configDirectory, config] of configs) {
		if (config.compilerOptions.paths) {
			configPaths.pushPaths(configDirectory, config.compilerOptions.paths);
		}
	}

	return configPaths;
}
/**
 * Takes a relative import and returns the absolute version of it, if possible.
 */
export function formatAsNonRelative(absoluteImportPath: string): string | undefined {
	const paths = findTsConfigPaths(absoluteImportPath);
	if (paths.empty) return;

	return paths.tryAliasImport(absoluteImportPath);
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
