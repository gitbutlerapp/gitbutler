import { execFileSync } from "node:child_process";
import { appendFileSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fixtureEnvironment, paths, type LiteTestEnvironment } from "./setup.ts";

/**
 * The seeded repository and its remote. Changes made through it end with the app
 * reloaded, so the test meets the app already showing them, unless a method says
 * otherwise.
 */
export class Repo {
	readonly clone: string;
	private readonly remote: string;
	private readonly env: Record<string, string>;
	private readonly reloadApp: () => Promise<void>;

	constructor(environment: LiteTestEnvironment, reloadApp: () => Promise<void>) {
		this.reloadApp = reloadApp;
		this.clone = path.join(environment.workdir, "local-clone");
		this.remote = path.join(environment.workdir, "remote-project");
		this.env = fixtureEnvironment(environment);
	}

	/** Add an uncommitted file to the worktree. */
	async write(file: string, content = "an uncommitted file\n"): Promise<void> {
		const target = path.join(this.clone, file);
		mkdirSync(path.dirname(target), { recursive: true });
		writeFileSync(target, content);
		await this.reloadApp();
	}

	read(file: string): string {
		return readFileSync(path.join(this.clone, file), "utf8");
	}

	/** Resolve a conflicted file the way an editor would, without reloading the app. */
	resolveConflictMarkers(file: string): void {
		const resolved = this.read(file)
			.split("\n")
			.filter((line) => !/^(<{7}|\|{7}|={7}|>{7})/.test(line))
			.join("\n");
		writeFileSync(path.join(this.clone, file), resolved);
	}

	/** The id of the commit with `subject`, on any ref. */
	commitWithSubject(subject: string): string {
		return this.git(
			this.clone,
			"log",
			"--all",
			"--format=%H",
			"--fixed-strings",
			`--grep=${subject}`,
		);
	}

	get objectFormat(): string {
		return this.git(this.clone, "rev-parse", "--show-object-format");
	}

	async createBranch(name: string, startPoint: string): Promise<void> {
		this.git(this.clone, "branch", name, startPoint);
		await this.reloadApp();
	}

	/** Subjects of `branch`'s commits, newest first. */
	subjects(branch: string): Array<string> {
		return this.git(this.clone, "log", "--format=%s", `refs/heads/${branch}`).split("\n");
	}

	tip(branch: string): string {
		return this.git(this.clone, "rev-parse", `refs/heads/${branch}`);
	}

	remoteTip(branch: string): string {
		return this.git(this.clone, "ls-remote", "origin", `refs/heads/${branch}`).split("\t")[0] ?? "";
	}

	/**
	 * branch1 applied and diverged: one commit only here, two only on the remote,
	 * both sides editing `a_file` so a rebase conflicts.
	 */
	async divergeBranch1(): Promise<void> {
		this.but("apply", "branch1");
		// branch1 is not checked out in the remote, so it can be rewritten in place.
		this.git(this.remote, "checkout", "branch1");
		this.git(this.remote, "reset", "--hard", "HEAD~1");
		appendFileSync(path.join(this.remote, "a_file"), "reworked upstream\n");
		this.git(this.remote, "commit", "-am", "Rework the parser entry point");
		writeFileSync(path.join(this.remote, "upstream_docs.md"), "docs for the rework\n");
		this.git(this.remote, "add", ".");
		this.git(this.remote, "commit", "-m", "Document the reworked entry point");
		this.git(this.remote, "checkout", "master");
		this.git(this.clone, "fetch", "origin");
		await this.reloadApp();
	}

	/** The amend-shaped divergence: branch1's tip reworded locally. */
	async rewriteBranch1Tip(): Promise<void> {
		this.rewordBranch1Tip();
		await this.reloadApp();
	}

	/**
	 * Both kinds: the tip reworded locally while the remote gained a commit on top
	 * of the original, so combining must not land the rewritten one twice.
	 */
	async divergeBoth(): Promise<void> {
		this.rewordBranch1Tip();
		this.git(this.remote, "checkout", "branch1");
		writeFileSync(path.join(this.remote, "upstream_notes.md"), "notes from upstream\n");
		this.git(this.remote, "add", ".");
		this.git(this.remote, "commit", "-m", "Add upstream notes");
		this.git(this.remote, "checkout", "master");
		this.git(this.clone, "fetch", "origin");
		await this.reloadApp();
	}

	private rewordBranch1Tip(): void {
		this.but("apply", "branch1");
		this.but("reword", this.tip("branch1"), "-m", "Reworded locally");
	}

	private git(cwd: string, ...args: Array<string>): string {
		return execFileSync("git", args, { cwd, env: this.env, encoding: "utf8" }).trim();
	}

	private but(...args: Array<string>): void {
		execFileSync(paths.but, args, { cwd: this.clone, env: this.env });
	}
}
