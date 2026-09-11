/** @vitest-environment jsdom */
import "fake-indexeddb/auto";
import * as idb from "idb-keyval";
import { QueryClient, QueryClientProvider, QueryObserver } from "@tanstack/react-query";
import { Suspense, act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, expect, it, vi } from "vitest";
import { currentForgeLoginQueryOptions, guiSettingsQueryOptions } from "./api/queries.ts";
import { defaultSettings } from "./settings.ts";
import { useSeenOnArrival } from "./review-seen.ts";
import { reviewStateQueryOptions, updateReviewState, watchReviewState } from "./review-state.ts";

globalThis.IS_REACT_ACT_ENVIRONMENT = true;
afterEach(() => vi.restoreAllMocks());
let projectNumber = 0;
const project = () => `migration-${projectNumber++}`;
const earlier = "2026-09-01T10:00:00Z";
const later = "2026-09-02T10:00:00Z";

it("migrates validated legacy records before deleting the old keys", async () => {
	const projectId = project();
	localStorage.setItem(
		`pr_activity_seen:v1:${projectId}`,
		JSON.stringify({ 7: earlier, bad: later, 8: "invalid" }),
	);
	localStorage.setItem(
		`pr_activity_unseen:v1:${projectId}`,
		JSON.stringify({
			7: [
				["comment:1", later],
				["bad", "invalid"],
			],
		}),
	);
	localStorage.setItem(`pr_activity_inbox:v1:${projectId}`, "malformed");
	const options = reviewStateQueryOptions(projectId);
	const state = await new QueryClient().fetchQuery(options);
	expect(state).toEqual({
		marks: { 7: earlier },
		unseen: { 7: [["comment:1", later]] },
		inbox: [],
	});
	expect(await idb.get(`pr_activity:v1:${projectId}`)).toEqual(state);
	for (const part of ["seen", "unseen", "inbox"])
		expect(localStorage.getItem(`pr_activity_${part}:v1:${projectId}`)).toBeNull();
	expect(await new QueryClient().fetchQuery(options)).toEqual(state);
});

it("preserves concurrent migrations and updates from stale windows", async () => {
	const projectId = project();
	const first = new QueryClient();
	const second = new QueryClient();
	localStorage.setItem(`pr_activity_seen:v1:${projectId}`, JSON.stringify({ 7: earlier }));
	const options = reviewStateQueryOptions(projectId);
	await Promise.all([first.fetchQuery(options), second.fetchQuery(options)]);
	await Promise.all([
		updateReviewState(first, projectId, (state) => ({
			...state,
			marks: { ...state.marks, 8: later },
		})),
		updateReviewState(second, projectId, (state) => ({
			...state,
			marks: { ...state.marks, 9: later },
		})),
	]);
	expect((await new QueryClient().fetchQuery(options)).marks).toEqual({
		7: earlier,
		8: later,
		9: later,
	});
});

it("invalidates another window and leaves unrelated row selectors unchanged", async () => {
	const projectId = project();
	const first = new QueryClient();
	const second = new QueryClient();
	const stops = [watchReviewState(first), watchReviewState(second)];
	const options = reviewStateQueryOptions(projectId);
	await second.fetchQuery(options);
	const observer = new QueryObserver(second, {
		...options,
		select: (state) => state.marks[7] ?? null,
		notifyOnChangeProps: ["data"],
	});
	const changed = vi.fn();
	const unsubscribe = observer.subscribe(changed);
	try {
		await updateReviewState(first, projectId, (state) => ({ ...state, marks: { 8: earlier } }));
		await vi.waitFor(() => expect(second.getQueryData(options.queryKey)?.marks[8]).toBe(earlier));
		expect(changed).not.toHaveBeenCalled();
		await updateReviewState(first, projectId, (state) => ({
			...state,
			marks: { ...state.marks, 7: later },
		}));
		await vi.waitFor(() => expect(observer.getCurrentResult().data).toBe(later));
	} finally {
		unsubscribe();
		stops.forEach((stop) => stop());
		first.clear();
		second.clear();
	}
});

it("keeps legacy data and session updates if IndexedDB is unavailable", async () => {
	const projectId = project();
	const client = new QueryClient();
	const options = reviewStateQueryOptions(projectId);
	localStorage.setItem(`pr_activity_seen:v1:${projectId}`, JSON.stringify({ 7: earlier }));
	vi.spyOn(IDBDatabase.prototype, "transaction").mockImplementation(() => {
		throw new Error("unavailable");
	});
	expect((await client.fetchQuery(options)).marks[7]).toBe(earlier);
	await updateReviewState(client, projectId, (state) => ({ ...state, marks: { 7: later } }));
	await client.invalidateQueries({ queryKey: options.queryKey });
	expect((await client.fetchQuery(options)).marks[7]).toBe(later);
	expect(localStorage.getItem(`pr_activity_seen:v1:${projectId}`)).not.toBeNull();
});

it("waits for hydration before capturing the arrival watermark, then holds that snapshot", async () => {
	const projectId = project();
	const client = new QueryClient({
		defaultOptions: { queries: { staleTime: Infinity, retry: false } },
	});
	client.setQueryData(guiSettingsQueryOptions.queryKey, { version: 1, ...defaultSettings });
	client.setQueryData(currentForgeLoginQueryOptions(projectId).queryKey, "alice");
	localStorage.setItem(`pr_activity_seen:v1:${projectId}`, JSON.stringify({ 7: earlier }));
	const container = document.createElement("div");
	const root = createRoot(container);
	const Arrival = () => <span>{useSeenOnArrival(projectId, 7).sinceMs}</span>;
	try {
		await act(async () => {
			root.render(
				<QueryClientProvider client={client}>
					<Suspense fallback="Loading">
						<Arrival />
					</Suspense>
				</QueryClientProvider>,
			);
		});
		await act(async () => {
			await client.ensureQueryData(reviewStateQueryOptions(projectId));
		});
		expect(container.textContent).toBe(String(Date.parse(earlier)));
		await act(async () => {
			await updateReviewState(client, projectId, (state) => ({ ...state, marks: { 7: later } }));
		});
		expect(container.textContent).toBe(String(Date.parse(earlier)));
	} finally {
		await act(async () => root.unmount());
		client.clear();
	}
});
