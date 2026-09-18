import { UiState, uiStateSlice, type TerminalSettings } from "$lib/state/uiState.svelte";
import { configureStore } from "@reduxjs/toolkit";
import { describe, expect, test } from "vitest";
import type { AppDispatch } from "$lib/state/clientState.svelte";

const WARP: TerminalSettings = { identifier: "warp", displayName: "Warp", platform: "macos" };

describe("UiState", () => {
	test("set() with a $state proxy stores plain, serializable data", () => {
		const store = configureStore({ reducer: { uiState: uiStateSlice.reducer } });

		const cleanup = $effect.root(() => {
			const uiState = new UiState(
				{
					get current() {
						return store.getState().uiState;
					},
				},
				store.dispatch as AppDispatch,
			);

			// Mirrors GeneralSettings.svelte: terminalOptions is a $state array,
			// so the selected option is a deeply-reactive proxy.
			const terminalOptions = $state([{ ...WARP }]);
			const selected = terminalOptions.find((o) => o.identifier === "warp")!;

			uiState.global.defaultTerminal.set(selected);
		});
		cleanup();

		// The dispatch must survive Immer's auto-freeze (Object.freeze on a
		// $state proxy throws state_descriptors_fixed)...
		const entities = store.getState().uiState.entities;
		expect(entities["defaultTerminal"]?.value).toEqual(WARP);

		// ...and redux-persist must be able to JSON-serialize what was stored.
		expect(JSON.parse(JSON.stringify(entities)).defaultTerminal.value).toEqual(WARP);
	});
});
