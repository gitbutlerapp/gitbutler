<script lang="ts">
	import { setTheme, themeStore } from '$lib/utils/theme.svelte';
	import { Icon } from '@gitbutler/ui';

	// Get the current theme
	const currentTheme = $derived($themeStore);
</script>

<div class="theme-switcher">
	<button
		type="button"
		class="theme-switcher__button"
		class:active={currentTheme === 'light'}
		onclick={() => setTheme('light')}
		aria-label="Light theme"
	>
		<Icon name="light-theme" />
	</button>
	<button
		type="button"
		class="theme-switcher__button"
		class:active={currentTheme === 'system'}
		onclick={() => setTheme('system')}
		aria-label="System theme"
	>
		<Icon name="system-theme" />
	</button>
	<button
		type="button"
		class="theme-switcher__button"
		class:active={currentTheme === 'dark'}
		onclick={() => setTheme('dark')}
		aria-label="Dark theme"
	>
		<Icon name="dark-theme" />
	</button>
</div>

<style lang="postcss">
	.theme-switcher {
		display: flex;
		align-self: flex-start;
		border: 1px solid var(--clr-border-2);
		border-radius: 100px;
	}

	.theme-switcher__button {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 24px;
		height: 24px;
		color: var(--clr-text-3);
		cursor: pointer;
		transition: color 0.1s ease-in-out;

		&:hover {
			color: var(--clr-text-2);
		}

		&.active {
			color: var(--clr-text-1);
		}
	}
</style>
