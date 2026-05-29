import { IpcError, applyIpcFingerprint } from "$lib/error/reduxError";
import { describe, expect, test } from "vitest";
import type { ErrorEvent, EventHint } from "@sentry/sveltekit";

/**
 * Pins the `beforeSend` wiring: given an `IpcError` exception hint, the
 * hook must copy the precomputed fingerprint onto the outgoing event;
 * for any other shape it must be a no-op.
 *
 * Together with `reduxError.test.ts` (which pins the normaliser itself)
 * this covers the JS-side path end to end. The "does Sentry's SDK
 * actually invoke `beforeSend` with this hint shape" question is the
 * SDK contract — verify once manually against the development Sentry
 * project after a meaningful change to the SDK or this hook.
 */
describe("applyIpcFingerprint", () => {
	function emptyEvent(): ErrorEvent {
		return { type: undefined } as ErrorEvent;
	}

	test("copies IpcError.fingerprint onto the event", () => {
		const err = new IpcError({ message: "boom" }, "noop_command");
		const event = emptyEvent();
		const result = applyIpcFingerprint(event, {
			originalException: err,
		} as EventHint);
		expect(result.fingerprint).toEqual(["ipc", "noop_command", "boom"]);
	});

	test("clones the array so caller mutations don't bleed into the IpcError", () => {
		const err = new IpcError({ message: "boom" }, "cmd");
		const event = emptyEvent();
		applyIpcFingerprint(event, { originalException: err } as EventHint);
		expect(event.fingerprint).not.toBe(err.fingerprint);
		expect(event.fingerprint).toEqual([...err.fingerprint]);
	});

	test("leaves event.fingerprint unset for a non-IpcError exception", () => {
		const err = new Error("plain error, not ipc");
		const event = emptyEvent();
		applyIpcFingerprint(event, { originalException: err } as EventHint);
		expect(event.fingerprint).toBeUndefined();
	});

	test("leaves event.fingerprint unset when there is no hint", () => {
		const event = emptyEvent();
		applyIpcFingerprint(event);
		expect(event.fingerprint).toBeUndefined();
	});

	test("leaves event.fingerprint unset when hint.originalException is missing", () => {
		const event = emptyEvent();
		applyIpcFingerprint(event, {} as EventHint);
		expect(event.fingerprint).toBeUndefined();
	});

	test("two messages that differ only by path land in the same bucket", () => {
		const errA = new IpcError(
			{ message: 'Worktree changes would be overwritten by checkout: "dir/file-a.ts"' },
			"create_virtual_branch_from_branch",
		);
		const errB = new IpcError(
			{ message: 'Worktree changes would be overwritten by checkout: "pkg/sub/file-b.go"' },
			"create_virtual_branch_from_branch",
		);
		const a = emptyEvent();
		const b = emptyEvent();
		applyIpcFingerprint(a, { originalException: errA } as EventHint);
		applyIpcFingerprint(b, { originalException: errB } as EventHint);
		expect(a.fingerprint).toEqual(b.fingerprint);
	});

	test("distinct root causes from one command stay in distinct buckets", () => {
		const cmd = "create_virtual_branch_from_branch";
		const reflog = new IpcError({ message: "The reflog could not be created or updated" }, cmd);
		const worktree = new IpcError(
			{ message: 'Worktree changes would be overwritten by checkout: "file.txt"' },
			cmd,
		);
		const eventA = emptyEvent();
		const eventB = emptyEvent();
		applyIpcFingerprint(eventA, { originalException: reflog } as EventHint);
		applyIpcFingerprint(eventB, { originalException: worktree } as EventHint);
		expect(eventA.fingerprint).not.toEqual(eventB.fingerprint);
	});

	test("reads `fingerprint` off a plain Error too (RTK Query rebuild path)", () => {
		// Errors that go through RTK's `tauriBaseQuery` get flattened into a
		// plain `{name, message, code, fingerprint}` object and then rebuilt
		// in `reduxErrorToException` (logError.ts) as a generic `Error` with
		// `fingerprint` copied across. `applyIpcFingerprint` must still
		// detect that fingerprint even though the value is no longer an
		// `IpcError` instance.
		const rebuilt = Object.assign(new Error("Worktree changes would be overwritten"), {
			fingerprint: [
				"ipc",
				"create_virtual_branch_from_branch",
				"Worktree changes would be overwritten",
			],
		});
		const event = emptyEvent();
		applyIpcFingerprint(event, { originalException: rebuilt } as EventHint);
		expect(event.fingerprint).toEqual([
			"ipc",
			"create_virtual_branch_from_branch",
			"Worktree changes would be overwritten",
		]);
	});

	test("ignores a `fingerprint` field that isn't a string array", () => {
		// Defensive: arbitrary objects in the wild might happen to have a
		// `fingerprint` property of the wrong shape. We shouldn't crash and
		// shouldn't apply garbage as the Sentry fingerprint.
		const bogus = Object.assign(new Error("oops"), {
			fingerprint: { not: "an array" },
		});
		const event = emptyEvent();
		applyIpcFingerprint(event, { originalException: bogus } as EventHint);
		expect(event.fingerprint).toBeUndefined();
	});

	test("ignores an empty `fingerprint` array", () => {
		const bogus = Object.assign(new Error("oops"), { fingerprint: [] });
		const event = emptyEvent();
		applyIpcFingerprint(event, { originalException: bogus } as EventHint);
		expect(event.fingerprint).toBeUndefined();
	});
});
