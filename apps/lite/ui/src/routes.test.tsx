/** @vitest-environment jsdom */

import type { LiteElectronApi } from "#electron/ipc.ts";
import type { ProjectForFrontend } from "@gitbutler/but-sdk";
import { QueryClient } from "@tanstack/react-query";
import { createMemoryHistory } from "@tanstack/react-router";
import { beforeEach, describe, expect, it, vi } from "vitest";

/**
 * The project route arms the window's watcher subscription in its loader.
 * Switching projects reruns that loader without an `onLeave` in between, so
 * the tests here pin the invariant the host cannot enforce: one live
 * subscription, for the project on screen.
 */

// Only the ids are read on the way to the workspace.
let projects: Array<ProjectForFrontend> = [];
/** Live subscriptions by id, each with the project it watches. */
const live = new Map<string, string>();
let counter = 0;
/** Whether the window was ever left with no subscription at all. */
let wentDark = false;

/** Projects whose subscribe call hangs until `release` runs. */
const stalled = new Set<string>();
const released: Array<() => void> = [];
const release = () => released.splice(0).forEach((resolve) => resolve());

const lite = {
	listProjectsStateless: () => Promise.resolve(projects),
	watcherSubscribe: (projectId: string) =>
		new Promise<string>((resolve) => {
			counter += 1;
			const id = `subscription-${counter}`;
			const subscribe = () => {
				live.set(id, projectId);
				resolve(id);
			};
			if (stalled.has(projectId)) released.push(subscribe);
			else subscribe();
		}),
	watcherUnsubscribe: (id: string) => {
		const removed = live.delete(id);
		if (live.size === 0) wentDark = true;
		return Promise.resolve(removed);
	},
} satisfies Partial<LiteElectronApi>;
Object.assign(window, { lite });

const openProject = async (id: string) => {
	// The route objects, and the subscription kept beside them, are module
	// singletons: fresh modules keep one test's subscription out of the next.
	const [{ createAppRouter }, { createRouteTree }] = await Promise.all([
		import("#ui/router.ts"),
		import("#ui/routes.tsx"),
	]);
	const router = createAppRouter(
		new QueryClient(),
		createRouteTree({ workspace: () => null }),
		createMemoryHistory({ initialEntries: [`/project/${id}/workspace`] }),
	);
	await router.load();
	return router;
};

const watching = () => [...live.values()];

// A cold runner spends most of a test's time on the fresh module graph.
describe("project watcher subscription", { timeout: 15_000 }, () => {
	beforeEach(() => {
		vi.resetModules();
		projects = [{ id: "a" }, { id: "b" }] as Array<ProjectForFrontend>;
		live.clear();
		counter = 0;
		wentDark = false;
		stalled.clear();
		released.length = 0;
	});

	it("follows the project across switches, one subscription at a time", async () => {
		const router = await openProject("a");
		expect(watching()).toEqual(["a"]);

		for (const id of ["b", "a", "b"]) {
			await router.navigate({ to: "/project/$id/workspace", params: { id } });
			await vi.waitFor(() => expect(watching()).toEqual([id]));
		}
	});

	it("keeps one subscription when the current project is opened again", async () => {
		const router = await openProject("a");
		await router.navigate({ to: "/project/$id/workspace", params: { id: "a" } });
		await router.navigate({ to: "/project/$id/workspace", params: { id: "a" } });
		await vi.waitFor(() => expect(watching()).toEqual(["a"]));
		// The watcher was handed over, never stopped and restarted.
		expect(wentDark).toBe(false);
	});

	it("settles on the project on screen when switches overlap", async () => {
		const router = await openProject("a");

		// B's watcher is slow to start; the user is back on A before it has.
		stalled.add("b");
		const toB = router.navigate({ to: "/project/$id/workspace", params: { id: "b" } });
		await vi.waitFor(() => expect(released).toHaveLength(1));
		const toA = router.navigate({ to: "/project/$id/workspace", params: { id: "a" } });
		release();
		await Promise.allSettled([toB, toA]);

		await vi.waitFor(() => expect(watching()).toEqual(["a"]));
	});

	it("drops the subscription on leaving the project", async () => {
		const router = await openProject("a");
		expect(watching()).toEqual(["a"]);

		// With no projects the index page stays put instead of redirecting back.
		projects = [];
		await router.navigate({ to: "/" });
		await vi.waitFor(() => expect(watching()).toEqual([]));
	});
});
