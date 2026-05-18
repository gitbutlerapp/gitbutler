import { butOk } from "./but.ts";
import { execFile } from "node:child_process";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import os from "node:os";

export interface TestRepo {
	/** Absolute path to the local clone. */
	dir: string;
	/** The unique prefix used for branch names in this test run. */
	prefix: string;
	/** Run a git command in the repo. */
	git(...args: string[]): Promise<string>;
	/** Run `but` in the repo. */
	but(...args: string[]): Promise<string>;
	/** Create a file and stage it. */
	createFile(relativePath: string, content: string): Promise<void>;
	/** Clean up: close any open PRs/MRs for our prefix, delete remote branches, remove temp dir. */
	cleanup(): Promise<void>;
}

interface TestRepoOptions {
	/** Full clone URL, e.g. https://github.com/owner/repo.git */
	cloneUrl: string;
	/** Git credentials to embed in the URL or configure via env. */
	token: string;
	/** Forge type for cleanup — "github" or "gitlab". */
	forge: "github" | "gitlab";
	/** owner/repo for API calls. */
	ownerRepo: string;
}

function git(args: string[], cwd: string, env?: Record<string, string>): Promise<string> {
	return new Promise((resolve, reject) => {
		execFile(
			"git",
			args,
			{
				cwd,
				env: { ...process.env, ...env },
				maxBuffer: 10 * 1024 * 1024,
				timeout: 60_000,
			},
			(error, stdout, stderr) => {
				if (error) {
					reject(
						new Error(`git ${args.join(" ")} failed:\n${stderr}\n${stdout}`),
					);
				} else {
					resolve(stdout.trim());
				}
			},
		);
	});
}

/**
 * Clone a sandbox repo into a temp directory and initialize `but` in it.
 *
 * Each test run gets a unique branch-name prefix to avoid collisions.
 */
export async function createTestRepo(opts: TestRepoOptions): Promise<TestRepo> {
	const prefix = `e2e-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
	const tmpDir = await mkdtemp(path.join(os.tmpdir(), "but-e2e-"));

	// Create an isolated app data dir for this test run.
	// Setting E2E_TEST_APP_DATA_DIR makes `but` store forge settings here
	// and also switches the secret backend to git-credentials (no system keychain needed).
	const appDataDir = path.join(tmpDir, ".but-app-data");
	await mkdir(appDataDir, { recursive: true });

	const butEnv: Record<string, string> = {
		E2E_TEST_APP_DATA_DIR: appDataDir,
	};

	// Configure git credential-store so secrets persist to a plain file.
	// This is what `but-secret` uses when git-credentials mode is active.
	const credentialStorePath = path.join(appDataDir, "git-credentials");
	await git(
		["config", "--global", "credential.helper", `store --file=${credentialStorePath}`],
		os.tmpdir(),
	);

	// Construct authenticated clone URL.
	const authedUrl = authenticatedUrl(opts.cloneUrl, opts.token, opts.forge);

	// Clone into the temp directory.
	await git(["clone", authedUrl, tmpDir], os.tmpdir());

	// Configure git identity for commits.
	await git(["config", "user.email", "e2e-bot@gitbutler.com"], tmpDir);
	await git(["config", "user.name", "GitButler E2E Bot"], tmpDir);

	// Initialize GitButler in the cloned repo.
	await butOk(["setup"], tmpDir, butEnv);

	// Authenticate `but` with the forge using the non-interactive --token flag.
	await butOk(
		["config", "forge", "auth", "--provider", opts.forge, "--token", opts.token],
		tmpDir,
		butEnv,
	);

	const repo: TestRepo = {
		dir: tmpDir,
		prefix,

		async git(...args: string[]) {
			return git(args, tmpDir);
		},

		async but(...args: string[]) {
			return butOk(args, tmpDir, butEnv);
		},

		async createFile(relativePath: string, content: string) {
			const fullPath = path.join(tmpDir, relativePath);
			await writeFile(fullPath, content, "utf-8");
		},

		async cleanup() {
			// Delete remote branches matching our prefix.
			try {
				const refs = await git(
					["for-each-ref", "--format=%(refname:short)", "refs/remotes/origin/"],
					tmpDir,
				);
				const branchesToDelete = refs
					.split("\n")
					.map((b) => b.replace("origin/", ""))
					.filter((b) => b.startsWith(prefix));

				for (const branch of branchesToDelete) {
					try {
						await git(["push", "origin", "--delete", branch], tmpDir);
					} catch {
						// Branch may already be deleted — ignore.
					}
				}
			} catch {
				// Ignore cleanup failures.
			}

			// Remove temp directory.
			await rm(tmpDir, { recursive: true, force: true });
		},
	};

	return repo;
}

function authenticatedUrl(
	cloneUrl: string,
	token: string,
	forge: "github" | "gitlab",
): string {
	const url = new URL(cloneUrl);
	if (forge === "github") {
		url.username = "x-access-token";
		url.password = token;
	} else {
		url.username = "oauth2";
		url.password = token;
	}
	return url.toString();
}
