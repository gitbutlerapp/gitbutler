import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { spawn } from "node:child_process";

/** @type {string | undefined} */
const mode = process.argv[2];
/** @type {string | undefined} */
const command = process.argv[3];
const commandArgs = process.argv.slice(4);

if (!mode || !command) {
	console.error("Usage: node scripts/run-desktop-tauri-with-env.mjs <mode> <command> [args...]");
	process.exit(1);
}

const repoRoot = process.cwd();
const desktopDir = resolve(repoRoot, "apps/desktop");
const envFiles = [
	resolve(desktopDir, ".env"),
	resolve(desktopDir, ".env.local"),
	resolve(desktopDir, `.env.${mode}`),
	resolve(desktopDir, `.env.${mode}.local`),
];

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

const child = spawn(command, commandArgs, {
	cwd: repoRoot,
	env: mergedEnv,
	stdio: "inherit",
	shell: process.platform === "win32",
});

child.on("exit", (code, signal) => {
	if (signal) {
		process.kill(process.pid, signal);
		return;
	}
	process.exit(code ?? 0);
});

child.on("error", (error) => {
	console.error(error);
	process.exit(1);
});

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
