/**
 * @file Which of a branch's tabs, diff or pull request, the user last picked.
 *
 * One choice across every branch and project, kept in local storage so it
 * survives a relaunch: a viewing preference, not a fact about any branch.
 */

import { useSyncExternalStore } from "react";

export type BranchTab = "diff" | "pr";

const storageKey = "branch_tab:v1";
const listeners = new Set<() => void>();

const read = (): BranchTab | null => {
	try {
		const stored = localStorage.getItem(storageKey);
		return stored === "diff" || stored === "pr" ? stored : null;
	} catch {
		return null;
	}
};

let chosen = read();

const subscribe = (listener: () => void): (() => void) => {
	listeners.add(listener);
	return () => {
		listeners.delete(listener);
	};
};

export const writeBranchTab = (tab: BranchTab): void => {
	if (chosen === tab) return;
	chosen = tab;
	try {
		localStorage.setItem(storageKey, tab);
	} catch {
		// The in-memory copy still serves this session.
	}
	for (const listener of listeners) listener();
};

/** The last tab picked, or `null` before any pick, when each surface leads with its own default. */
export const useChosenBranchTab = (): BranchTab | null =>
	useSyncExternalStore(subscribe, () => chosen);
