import type { IBackend } from "$lib/backend";
import { applyThemeToDocument, type AppTheme } from "$lib/theme/themes";
import type { UiState } from "$lib/state/uiState.svelte";

let systemTheme: string | null;
let selectedTheme: AppTheme | undefined;

export function initTheme(uiState: UiState, backend: IBackend) {
	backend.systemTheme.subscribe((theme) => {
		systemTheme = theme;
		updateDom();
	});
	// $effect.root is needed because initTheme runs outside a component context.
	$effect.root(() => {
		$effect(() => {
			selectedTheme = uiState.global.theme.current;
			updateDom();
		});
	});
}

function updateDom() {
	applyThemeToDocument(document.documentElement, selectedTheme, systemTheme);
}
