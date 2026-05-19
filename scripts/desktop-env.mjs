import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

/**
 * @param {string} mode
 * @param {{ repoRoot?: string, includeRoot?: boolean }} [options]
 * @returns {NodeJS.ProcessEnv}
 */
export function mergeDesktopEnv(mode, options = {}) {
	const repoRoot = options.repoRoot ?? process.cwd();
	const desktopDir = resolve(repoRoot, "apps/desktop");
	const envFiles = [];

	if (options.includeRoot) {
		envFiles.push(
			resolve(repoRoot, ".env"),
			resolve(repoRoot, ".env.local"),
			resolve(repoRoot, `.env.${mode}`),
			resolve(repoRoot, `.env.${mode}.local`),
		);
	}

	envFiles.push(
		resolve(desktopDir, ".env"),
		resolve(desktopDir, ".env.local"),
		resolve(desktopDir, `.env.${mode}`),
		resolve(desktopDir, `.env.${mode}.local`),
	);

	/** @type {NodeJS.ProcessEnv} */
	const mergedEnv = { ...process.env };

	for (const filePath of envFiles) {
		if (!existsSync(filePath)) continue;
		const fileEnv = parseDotEnv(readFileSync(filePath, "utf8"));
		for (const [key, value] of Object.entries(fileEnv)) {
			if (process.env[key] === undefined) {
				mergedEnv[key] = value;
			}
		}
	}

	if (
		mergedEnv.TAURI_SIGNING_PRIVATE_KEY === undefined &&
		mergedEnv.TAURI_PRIVATE_KEY !== undefined
	) {
		mergedEnv.TAURI_SIGNING_PRIVATE_KEY = mergedEnv.TAURI_PRIVATE_KEY;
	}

	if (
		mergedEnv.TAURI_SIGNING_PRIVATE_KEY_PASSWORD === undefined &&
		mergedEnv.TAURI_KEY_PASSWORD !== undefined
	) {
		mergedEnv.TAURI_SIGNING_PRIVATE_KEY_PASSWORD = mergedEnv.TAURI_KEY_PASSWORD;
	}

	return mergedEnv;
}

/**
 * @param {string} source
 * @returns {Record<string, string>}
 */
function parseDotEnv(source) {
	/** @type {Record<string, string>} */
	const vars = {};

	for (const rawLine of source.split(/\r?\n/)) {
		const line = rawLine.trim();
		if (!line || line.startsWith("#")) continue;

		const match = line.match(/^(?:export\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.*)$/);
		if (!match) continue;

		const [, key, rawValue] = match;
		vars[key] = normalizeValue(rawValue);
	}

	return vars;
}

/**
 * @param {string} rawValue
 * @returns {string}
 */
function normalizeValue(rawValue) {
	const trimmed = rawValue.trim();

	if (
		(trimmed.startsWith('"') && trimmed.endsWith('"')) ||
		(trimmed.startsWith("'") && trimmed.endsWith("'"))
	) {
		const unquoted = trimmed.slice(1, -1);
		return trimmed.startsWith('"')
			? unquoted
					.replace(/\\n/g, "\n")
					.replace(/\\r/g, "\r")
					.replace(/\\t/g, "\t")
					.replace(/\\"/g, '"')
					.replace(/\\\\/g, "\\")
			: unquoted;
	}

	const commentStart = trimmed.indexOf(" #");
	return commentStart >= 0 ? trimmed.slice(0, commentStart).trimEnd() : trimmed;
}
