import { UiState, uiStateSlice } from "$lib/state/uiState.svelte";
import { reactive } from "@gitbutler/shared/reactiveUtils.svelte";
import { configureStore } from "@reduxjs/toolkit";
import { flushSync } from "svelte";
import { expect, test } from "vitest";

test("language preference survives restoring Redux state and can return to system default", () => {
	const destroy = $effect.root(() => {
		const store = configureStore({ reducer: uiStateSlice.reducer });
		let state = $state.raw(store.getState());
		const unsubscribe = store.subscribe(() => (state = store.getState()));
		const uiState = new UiState(
			reactive(() => state),
			store.dispatch,
		);
		flushSync();
		expect(uiState.global.locale.current).toBeUndefined();
		uiState.global.locale.set("zh-TW");
		flushSync();

		const restoredStore = configureStore({
			reducer: uiStateSlice.reducer,
			preloadedState: JSON.parse(JSON.stringify(store.getState())),
		});
		let restoredState = $state.raw(restoredStore.getState());
		const unsubscribeRestored = restoredStore.subscribe(
			() => (restoredState = restoredStore.getState()),
		);
		const restored = new UiState(
			reactive(() => restoredState),
			restoredStore.dispatch,
		);
		flushSync();
		expect(restored.global.locale.current).toBe("zh-TW");
		restored.global.locale.set("en");
		flushSync();
		expect(restored.global.locale.current).toBe("en");
		restored.global.locale.set(undefined);
		flushSync();
		expect(restored.global.locale.current).toBeUndefined();
		return () => {
			unsubscribe();
			unsubscribeRestored();
		};
	});
	destroy();
});
