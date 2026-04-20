/**
 * Cross-platform helpers for e2e setup scripts.
 *
 * Each script is executed by Node with `--experimental-strip-types`,
 * so plain TypeScript (no enums / const-enums / namespaces) is fine.
 */
import { execFileSync, type ExecFileSyncOptions } from "node:child_process";
import { appendFileSync, mkdirSync, writeFileSync, chmodSync } from "node:fs";
import path from "node:path";

/* ------------------------------------------------------------------ */
/*  Environment                                                       */
/* ------------------------------------------------------------------ */

export const BUT_TESTING: string = process.env.BUT_TESTING ?? "";
export const GIT_CONFIG_GLOBAL: string = process.env.GIT_CONFIG_GLOBAL ?? "";
export const GITBUTLER_CLI_DATA_DIR: string = process.env.GITBUTLER_CLI_DATA_DIR ?? "";

/** Script arguments (after `node <script>`). */
export const args: string[] = process.argv.slice(2);

/* ------------------------------------------------------------------ */
/*  Logging                                                           */
/* ------------------------------------------------------------------ */

export function logEnv(): void {
	console.log(`GIT CONFIG ${GIT_CONFIG_GLOBAL}`);
	console.log(`DATA DIR ${GITBUTLER_CLI_DATA_DIR}`);
	console.log(`BUT_TESTING ${BUT_TESTING}`);
}

/* ------------------------------------------------------------------ */
/*  File helpers                                                      */
/* ------------------------------------------------------------------ */

export function mkdir(dir: string): void {
	mkdirSync(dir, { recursive: true });
}

/** Append text followed by a newline, like `echo "text" >> file`. */
export function appendLine(file: string, text: string): void {
	appendFileSync(file, text + "\n");
}

/** Write text to a file, like `echo "text" > file`. */
export function writeLine(file: string, text: string): void {
	writeFileSync(file, text + "\n");
}

/** Create an empty file, like `touch file`. */
export function touch(file: string): void {
	writeFileSync(file, "");
}

/** Make a file executable (no-op on Windows where it isn't needed). */
export function makeExecutable(file: string): void {
	if (process.platform !== "win32") {
		chmodSync(file, 0o755);
	}
}

/* ------------------------------------------------------------------ */
/*  Command execution                                                 */
/* ------------------------------------------------------------------ */

const defaultExecOpts: ExecFileSyncOptions = { stdio: "inherit" };

/** Run an arbitrary command synchronously, inheriting stdio. */
export function run(cmd: string, cmdArgs: string[], cwd?: string): void {
	execFileSync(cmd, cmdArgs, { ...defaultExecOpts, cwd });
}

/** Shorthand for running git commands. */
export function git(...gitArgs: string[]): void {
	execFileSync("git", gitArgs, defaultExecOpts);
}

/** Run git in a specific directory. */
export function gitIn(cwd: string, ...gitArgs: string[]): void {
	execFileSync("git", gitArgs, { ...defaultExecOpts, cwd });
}

/** Run git and capture stdout (trimmed). */
export function gitOutput(...gitArgs: string[]): string {
	return execFileSync("git", gitArgs, { encoding: "utf-8" }).trim();
}

/** Run git in a specific directory and capture stdout (trimmed). */
export function gitOutputIn(cwd: string, ...gitArgs: string[]): string {
	return execFileSync("git", gitArgs, { cwd, encoding: "utf-8" }).trim();
}

/** Run the but-testing binary. */
export function butTesting(...btArgs: string[]): void {
	if (!BUT_TESTING) {
		throw new Error("BUT_TESTING environment variable is not set");
	}
	execFileSync(BUT_TESTING, btArgs, defaultExecOpts);
}

/** Run the but-testing binary in a specific directory. */
export function butTestingIn(cwd: string, ...btArgs: string[]): void {
	if (!BUT_TESTING) {
		throw new Error("BUT_TESTING environment variable is not set");
	}
	execFileSync(BUT_TESTING, btArgs, { ...defaultExecOpts, cwd });
}

/* ------------------------------------------------------------------ */
/*  Directory helpers                                                 */
/* ------------------------------------------------------------------ */

const dirStack: string[] = [];

/** Change to `dir`, pushing the current directory onto a stack. */
export function pushd(dir: string): void {
	dirStack.push(process.cwd());
	process.chdir(dir);
}

/** Return to the directory saved by the last `pushd`. */
export function popd(): void {
	const prev = dirStack.pop();
	if (prev === undefined) {
		throw new Error("popd: directory stack is empty");
	}
	process.chdir(prev);
}

/* ------------------------------------------------------------------ */
/*  Git hook helpers                                                  */
/* ------------------------------------------------------------------ */

/** Write a git hook file and make it executable. */
export function writeHook(hooksDir: string, hookName: string, content: string): void {
	const hookPath = path.join(hooksDir, hookName);
	writeFileSync(hookPath, content);
	makeExecutable(hookPath);
}
