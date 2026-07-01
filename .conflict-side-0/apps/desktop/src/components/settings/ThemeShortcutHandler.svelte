<script lang="ts">
	import { BACKEND } from "$lib/backend";
	import { initTheme } from "$lib/bootstrap/theme.svelte";
	import { SHORTCUT_SERVICE } from "$lib/shortcuts/shortcutService";
	import { UI_STATE } from "$lib/state/uiState.svelte";
	import { inject } from "@gitbutler/core/context";

	const uiState = inject(UI_STATE);
	const shortcutService = inject(SHORTCUT_SERVICE);
	const backend = inject(BACKEND);
	const theme = uiState.global.theme;

	initTheme(uiState, backend);

	function updateTheme() {
		theme.set(theme.current === "light" ? "dark" : "light");
	}

	$effect(() => shortcutService.on("switch-theme", () => updateTheme()));
</script>
