import { execFileSync } from "node:child_process";
import { appendFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fixtureEnvironment, paths, type LiteTestEnvironment } from "./setup.ts";

/**
 * branch1 applied and diverged: one commit only here, two only on the remote,
 * both sides editing `a_file` so a rebase conflicts. Reload the app afterwards.
 */
export const divergeBranch1 = (environment: LiteTestEnvironment): void => {
	const env = fixtureEnvironment(environment);
	const clone = path.join(environment.workdir, "local-clone");
	const remote = path.join(environment.workdir, "remote-project");
	const git = (cwd: string, ...args: Array<string>) => execFileSync("git", args, { cwd, env });

	execFileSync(paths.but, ["apply", "branch1"], { cwd: clone, env });

	// branch1 is not checked out in the remote, so it can be rewritten in place.
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
 * The amend-shaped divergence: branch1's tip reworded locally. Head info prunes
 * the remote's twin, so only the push status shows it.
 */
export const rewriteBranch1Tip = (environment: LiteTestEnvironment): void => {
	const env = fixtureEnvironment(environment);
	const clone = path.join(environment.workdir, "local-clone");

	execFileSync(paths.but, ["apply", "branch1"], { cwd: clone, env });
	const tip = execFileSync("git", ["-C", clone, "rev-parse", "refs/heads/branch1"], {
		encoding: "utf8",
	}).trim();
	execFileSync(paths.but, ["reword", tip, "-m", "Reworded locally"], { cwd: clone, env });
};

/**
 * Both kinds: the tip reworded locally while the remote gained a commit on top
 * of the original, so combining must not land the rewritten one twice.
 */
export const divergeBoth = (environment: LiteTestEnvironment): void => {
	rewriteBranch1Tip(environment);

	const env = fixtureEnvironment(environment);
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
