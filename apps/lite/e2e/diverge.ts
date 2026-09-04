import { execFileSync } from "node:child_process";
import { appendFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { paths, processEnvironment, type LiteTestEnvironment } from "./setup.ts";

/**
 * Leaves `local-clone` (of the `project-with-remote-branches.sh` scenario)
 * with branch1 applied and diverged from its upstream: the remote side of
 * branch1 is rewritten — its tip dropped and two different commits put in its
 * place — so the workspace holds one commit the remote lacks ("branch1:
 * second commit") and the remote holds two the workspace lacks, force push
 * required. The kept local commit and the remote rework both edit `a_file`,
 * so rebasing the local commit onto the remote conflicts.
 *
 * Node-side only: reload the app afterwards to see the divergence.
 */
export const divergeBranch1 = (environment: LiteTestEnvironment): void => {
	const but = process.env.BUT ?? path.join(paths.repoRoot, "target/debug/but");
	const env = processEnvironment({
		BUT: but,
		E2E_TEST_APP_DATA_DIR: environment.appDataDir,
		GIT_CONFIG_GLOBAL: environment.gitConfig,
	});
	const clone = path.join(environment.workdir, "local-clone");
	const remote = path.join(environment.workdir, "remote-project");
	const git = (cwd: string, ...args: Array<string>) => execFileSync("git", args, { cwd, env });

	execFileSync(but, ["apply", "branch1"], { cwd: clone, env });

	// The collaborator's side: remote-project is the clone's origin, and
	// branch1 is not its checked-out branch, so it can be rewritten in place.
	git(remote, "checkout", "branch1");
	git(remote, "reset", "--hard", "HEAD~1");
	appendFileSync(path.join(remote, "a_file"), "reworked upstream\n");
	git(remote, "commit", "-am", "Rework the parser entry point");
	writeFileSync(path.join(remote, "upstream_docs.md"), "docs for the rework\n");
	git(remote, "add", ".");
	git(remote, "commit", "-m", "Document the reworked entry point");
	git(remote, "checkout", "master");

	git(clone, "fetch", "origin");
};

/**
 * The amend-shaped divergence: branch1 applied and its tip reworded locally,
 * so local and remote hold different versions of the same change. Head info
 * prunes the remote's version as similar — `commitsOnRemote` stays empty and
 * only the force-required push status betrays the divergence.
 */
export const rewriteBranch1Tip = (environment: LiteTestEnvironment): void => {
	const but = process.env.BUT ?? path.join(paths.repoRoot, "target/debug/but");
	const env = processEnvironment({
		BUT: but,
		E2E_TEST_APP_DATA_DIR: environment.appDataDir,
		GIT_CONFIG_GLOBAL: environment.gitConfig,
	});
	const clone = path.join(environment.workdir, "local-clone");

	execFileSync(but, ["apply", "branch1"], { cwd: clone, env });
	const tip = execFileSync("git", ["-C", clone, "rev-parse", "refs/heads/branch1"], {
		encoding: "utf8",
	}).trim();
	execFileSync(but, ["reword", tip, "-m", "Reworded locally"], { cwd: clone, env });
};

/**
 * Both kinds at once: branch1 applied and its tip reworded locally, while
 * the remote gained a genuinely new commit on top of the original tip. So
 * the remote holds an older version of one local commit *and* a commit the
 * workspace lacks — the case where combining must bring the new commit in
 * without landing the rewritten one twice.
 */
export const divergeBoth = (environment: LiteTestEnvironment): void => {
	rewriteBranch1Tip(environment);

	const env = processEnvironment({
		E2E_TEST_APP_DATA_DIR: environment.appDataDir,
		GIT_CONFIG_GLOBAL: environment.gitConfig,
	});
	const clone = path.join(environment.workdir, "local-clone");
	const remote = path.join(environment.workdir, "remote-project");
	const git = (cwd: string, ...args: Array<string>) => execFileSync("git", args, { cwd, env });

	// The collaborator adds on top of the tip as the remote still has it.
	git(remote, "checkout", "branch1");
	writeFileSync(path.join(remote, "upstream_notes.md"), "notes from upstream\n");
	git(remote, "add", ".");
	git(remote, "commit", "-m", "Add upstream notes");
	git(remote, "checkout", "master");

	git(clone, "fetch", "origin");
};
