<script lang="ts">
	import { setTheme, themeStore } from "$lib/utils/theme.svelte";
	import { Icon } from "@gitbutler/ui";

	// Get the current theme
	const currentTheme = $derived($themeStore);
</script>

<div class="theme-switcher">
	<button
		type="button"
		class="theme-switcher__button"
		class:active={currentTheme === "light"}
		onclick={() => setTheme("light")}
		aria-label="Helles Design"
	>
		<Icon name="theme-light" />
	</button>
	<button
		type="button"
		class="theme-switcher__button"
		class:active={currentTheme === "system"}
		onclick={() => setTheme("system")}
		aria-label="System-Design"
	>
		<Icon name="theme-system" />
	</button>
	<button
		type="button"
		class="theme-switcher__button"
		class:active={currentTheme === "dark"}
		onclick={() => setTheme("dark")}
		aria-label="Dunkles Design"
	>
		<Icon name="theme-dark" />
	</button>
</div>

<style lang="postcss">
	.theme-switcher {
		display: flex;
		align-self: flex-start;
		border: 1px solid var(--border-2);
		border-radius: 100px;
	}

	.theme-switcher__button {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 24px;
		height: 24px;
		color: var(--text-3);
		cursor: pointer;
		transition: color 0.1s ease-in-out;

		&:hover {
			color: var(--text-2);
		}

		&.active {
			color: var(--text-1);
		}
	}
</style>
