import { IpcError } from "$lib/error/normalizedError";
import { describe, expect, test } from "vitest";

/**
 * The fingerprint shape is `["ipc", <command>, <normalised-first-line>]`.
 * These tests anchor the normaliser so a regex tweak that accidentally
 * un-strips a per-user identifier (paths, branch names, SHAs) fails CI
 * before it ships and silently re-broadens Sentry buckets.
 *
 * Identifiers in the sample messages below (file paths, branch names,
 * SHAs, UUIDs) are synthetic placeholders chosen to exercise the
 * relevant rule; they are not taken from any specific production event.
 */
describe("IpcError.fingerprint", () => {
	test('namespaces by ["ipc", command, normalised-message]', () => {
		const err = new IpcError({ message: "something went wrong" }, "noop_command");
		expect(err.fingerprint).toEqual(["ipc", "noop_command", "something went wrong"]);
	});

	test("only the first line of multi-line messages drives the bucket", () => {
		const err = new IpcError(
			{
				message:
					'The reflog could not be created or updated\n\nCaused by:\n    1: Could not open reflog file at "/Users/u/repo/.git"',
			},
			"apply",
		);
		expect(err.fingerprint[2]).toBe("The reflog could not be created or updated");
	});

	test("strips refs/heads/* so different branches bucket together", () => {
		const ref1 = new IpcError(
			{
				message:
					"Unexpectedly failed to find anchor for refs/heads/branch-one to make it a dependent branch",
			},
			"apply",
		);
		const ref2 = new IpcError(
			{
				message:
					"Unexpectedly failed to find anchor for refs/heads/branch-two/sub to make it a dependent branch",
			},
			"apply",
		);
		expect(ref1.fingerprint).toEqual(ref2.fingerprint);
		expect(ref1.fingerprint[2]).toContain("refs/<ref>");
	});

	test("strips relative paths inside checkout-conflict messages", () => {
		const a = new IpcError(
			{ message: 'Worktree changes would be overwritten by checkout: "dir/file-a.ts"' },
			"apply",
		);
		const b = new IpcError(
			{ message: 'Worktree changes would be overwritten by checkout: "pkg/sub/file-b.go"' },
			"apply",
		);
		expect(a.fingerprint).toEqual(b.fingerprint);
	});

	test("strips absolute paths so different users bucket together", () => {
		const a = new IpcError(
			{ message: "Refusing to delete a branch that is checked out. Worktree: /Users/u1/repo" },
			"delete_local_branch",
		);
		const b = new IpcError(
			{ message: "Refusing to delete a branch that is checked out. Worktree: /Users/u2/repo" },
			"delete_local_branch",
		);
		expect(a.fingerprint).toEqual(b.fingerprint);
		expect(a.fingerprint[2]).toContain("<path>");
	});

	test("strips relative paths so per-repo file targets bucket together", () => {
		const a = new IpcError(
			{ message: "Unable to determine target commit for unassigned change: dir-a/.gitignore" },
			"create_commit",
		);
		const b = new IpcError(
			{
				message: "Unable to determine target commit for unassigned change: dir-b/manifest.json",
			},
			"create_commit",
		);
		expect(a.fingerprint).toEqual(b.fingerprint);
		expect(a.fingerprint[2]).toContain("<path>");
	});

	test("strips UUIDs", () => {
		const err = new IpcError(
			{ message: "branch with ID 00000000-0000-0000-0000-000000000000 not found" },
			"unapply_stack",
		);
		expect(err.fingerprint[2]).toBe("branch with ID <uuid> not found");
	});

	test("strips git SHAs", () => {
		const a = new IpcError(
			{ message: "failed to uncommit commits: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" },
			"commit_uncommit",
		);
		const b = new IpcError(
			{ message: "failed to uncommit commits: bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb" },
			"commit_uncommit",
		);
		expect(a.fingerprint).toEqual(b.fingerprint);
		expect(a.fingerprint[2]).toBe("failed to uncommit commits: <hex>");
	});

	test("strips quoted branch names so different branches bucket together", () => {
		const a = new IpcError(
			{ message: "Branch 'branch-one' cannot be created: the target commit is not present" },
			"create_virtual_branch",
		);
		const b = new IpcError(
			{
				message: "Branch 'branch-two' cannot be created: the target commit is not present",
			},
			"create_virtual_branch",
		);
		expect(a.fingerprint).toEqual(b.fingerprint);
		expect(a.fingerprint[2]).toContain("'<id>'");
	});

	test("does not over-collapse: distinct root causes from the same endpoint stay distinct", () => {
		const command = "apply";
		const reflog = new IpcError({ message: "The reflog could not be created or updated" }, command);
		const worktree = new IpcError(
			{ message: 'Worktree changes would be overwritten by checkout: "file.txt"' },
			command,
		);
		const anchor = new IpcError(
			{
				message:
					"Unexpectedly failed to find anchor for refs/heads/branch-x to make it a dependent branch",
			},
			command,
		);
		const verification = new IpcError({ message: "<verification-failed>" }, command);
		const all = [reflog, worktree, anchor, verification].map((e) => e.fingerprint[2]);
		expect(new Set(all).size).toBe(4);
	});

	test("does not eat unrelated content: branch-applied-in-workspace stays untouched", () => {
		const err = new IpcError(
			{ message: "Cannot delete a branch that is applied in workspace" },
			"delete_local_branch",
		);
		expect(err.fingerprint[2]).toBe("Cannot delete a branch that is applied in workspace");
	});

	test("includes the command so unrelated endpoints can never collide", () => {
		const a = new IpcError({ message: "same words" }, "command_a");
		const b = new IpcError({ message: "same words" }, "command_b");
		expect(a.fingerprint).not.toEqual(b.fingerprint);
	});
});
