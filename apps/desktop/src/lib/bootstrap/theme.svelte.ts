import type { IBackend } from "$lib/backend";
import type { UiState } from "$lib/state/uiState.svelte";

let systemTheme: string | null;
let selectedTheme: string | undefined;

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
	const docEl = document.documentElement;
	if (
		selectedTheme === "dark" ||
		(selectedTheme === "system" && systemTheme === "dark") ||
		(selectedTheme === undefined && systemTheme === "dark")
	) {
		docEl.classList.remove("light");
		docEl.classList.add("dark");
		docEl.style.colorScheme = "dark";
	} else if (
		selectedTheme === "light" ||
		(selectedTheme === "system" && systemTheme === "light") ||
		(selectedTheme === undefined && systemTheme === "light")
	) {
		docEl.classList.remove("dark");
		docEl.classList.add("light");
		docEl.style.colorScheme = "light";
	}
}
