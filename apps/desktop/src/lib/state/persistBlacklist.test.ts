import {
	UNCOMMITTED_PERSIST_BLACKLIST,
	uncommittedActions,
	uncommittedSlice,
} from "$lib/selection/uncommitted";
import { persistConfigFor } from "$lib/state/persistConfig";
import { configureStore } from "@reduxjs/toolkit";
import { persistReducer } from "redux-persist";
import persistStore from "redux-persist/lib/persistStore";
import { describe, expect, test } from "vitest";
import type { Storage } from "redux-persist";

/** Records what redux-persist writes, so tests can assert on the persisted shape itself. */
function recordingStorage(seed?: Record<string, string>): Storage & {
	written(): Record<string, string>;
} {
	const items: Record<string, string> = { ...seed };
	return {
		getItem: async (key: string) => items[key] ?? null,
		setItem: async (key: string, value: string) => {
			items[key] = value;
		},
		removeItem: async (key: string) => {
			delete items[key];
		},
		written: () => items,
	};
}

type UncommittedState = ReturnType<typeof uncommittedSlice.getInitialState>;

const KEY = uncommittedSlice.reducerPath;
const BLACKLIST = [...UNCOMMITTED_PERSIST_BLACKLIST];

/** A change and its assignment, enough to populate every part of the slice. */
const CHANGE = { path: "a.txt", status: { type: "Modification" } } as never;
const ASSIGNMENT = {
	id: "assignment-1",
	path: "a.txt",
	pathBytes: "a.txt",
	stackId: null,
	hunkHeader: null,
	lineNumsAdded: null,
	lineNumsRemoved: null,
} as never;

/**
 * Run the real slice through a real persist cycle under the config the app uses, with only
 * storage swapped out, and report what reached storage and what the store holds.
 */
async function persistCycle(seed?: Record<string, string>) {
	const storage = recordingStorage(seed);
	const store = configureStore({
		reducer: persistReducer(
			{ ...persistConfigFor<UncommittedState>(KEY, BLACKLIST), storage },
			uncommittedSlice.reducer,
		),
		middleware: (getDefault) => getDefault({ serializableCheck: false }),
	});
	await new Promise<void>((resolve) => persistStore(store, undefined, () => resolve()));
	// What the app would render on startup: rehydrated, but before the worktree query lands.
	const rehydrated = store.getState();
	store.dispatch(uncommittedActions.update({ assignments: [ASSIGNMENT], changes: [CHANGE] }));
	// redux-persist writes on a timeout, so let the queued write run.
	await new Promise((resolve) => setTimeout(resolve, 50));

	const raw = storage.written()[`persist:${KEY}`];
	return {
		persisted: raw ? Object.keys(JSON.parse(raw)).filter((k) => k !== "_persist") : [],
		rehydrated,
	};
}

describe("uncommitted slice persistence", () => {
	// Types already stop a name that is not part of the slice; this stops the opposite mistake.
	test("the checkbox state is not blacklisted", () => {
		expect(BLACKLIST).not.toContain("hunkSelection");
	});

	test("only the checkbox state is written", async () => {
		const { persisted } = await persistCycle();
		expect(persisted).toEqual(["hunkSelection"]);
	});

	// Anything an earlier build already wrote has to be dropped on the way in as well, or the
	// first launch after upgrading rehydrates a stale file list and flashes it once more.
	test("state left by an earlier build is not rehydrated", async () => {
		const stale = JSON.stringify({
			treeChanges: JSON.stringify({ ids: ["gone.txt"], entities: { "gone.txt": CHANGE } }),
			hunkAssignments: JSON.stringify({ ids: [], entities: {} }),
			hunkSelection: JSON.stringify({ ids: [], entities: {} }),
			_persist: JSON.stringify({ version: -1, rehydrated: true }),
		});
		const { rehydrated } = await persistCycle({ [`persist:${KEY}`]: stale });
		expect(rehydrated.treeChanges.ids).toEqual([]);
	});
});
