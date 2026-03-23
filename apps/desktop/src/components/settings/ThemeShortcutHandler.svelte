<script lang="ts">
	import { BACKEND } from "$lib/backend";
	import { initTheme } from "$lib/bootstrap/theme";
	import { SETTINGS } from "$lib/settings/userSettings";
	import { SHORTCUT_SERVICE } from "$lib/shortcuts/shortcutService";
	import { inject } from "@gitbutler/core/context";

	const userSettings = inject(SETTINGS);
	const shortcutService = inject(SHORTCUT_SERVICE);
	const backend = inject(BACKEND);

	initTheme(userSettings, backend);

	function updateTheme() {
		userSettings.update((s) => ({
			...s,
			theme: s.theme === "light" ? "dark" : "light",
		}));
	}

	$effect(() => shortcutService.on("switch-theme", () => updateTheme()));
</script>
