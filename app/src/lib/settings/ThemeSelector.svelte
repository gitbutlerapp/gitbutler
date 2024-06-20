<script lang="ts">
	import Icon from '$lib/components/Icon.svelte';
	import type { Settings } from '$lib/settings/userSettings';
	import type { Writable } from 'svelte/store';

	export let userSettings: Writable<Settings>;

	const themes = [
		{
			name: 'Light',
			value: 'light',
			preview: '/images/theme-previews/light.svg'
		},
		{
			name: 'Dark',
			value: 'dark',
			preview: '/images/theme-previews/dark.svg'
		},
		{
			name: 'System preference',
			value: 'system',
			preview: '/images/theme-previews/system.svg'
		}
	];
</script>

<fieldset class="cards-group">
	{#each themes as theme}
		<label
			class="theme-card"
			class:selected={theme.value === $userSettings.theme}
			for="theme-{theme.value}"
		>
			<input
				class="hidden-input"
				type="radio"
				id="theme-{theme.value}"
				value={$userSettings.theme || 'system'}
				checked={theme.value === $userSettings.theme}
				on:change={() => userSettings.update((s) => ({ ...s, theme: theme.value }))}
			/>
			<div class="theme-card__preview">
				<i class="theme-card__icon"><Icon name="success" color="pop" /></i>

				<img src={theme.preview} alt={theme.name} />
			</div>

			<span class="theme-card__label text-base-12 text-semibold">{theme.name}</span>
		</label>
	{/each}
</fieldset>

<style lang="postcss">
	.cards-group {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		gap: 16px;
	}

	.theme-card {
		cursor: pointer;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 8px;
	}

	.theme-card:hover {
		& .theme-card__label {
			background-color: var(--clr-bg-2);
		}
	}

	.theme-card__preview {
		position: relative;
		width: 100%;
		height: auto;
		border-radius: var(--radius-m);
		border: 1px solid var(--clr-border-2);
		overflow: hidden;

		& img {
			width: 100%;
			height: auto;
			border-radius: var(--radius-m);
		}
	}

	.theme-card__label {
		padding: 6px;
		text-align: center;
		border-radius: var(--radius-m);
	}

	.theme-card__icon {
		z-index: 1;
		position: absolute;
		bottom: 6px;
		right: 6px;
		opacity: 0;
	}

	.hidden-input {
		position: absolute;
		width: 0;
		height: 0;
		z-index: -1;
	}

	/* MODIFIER */

	.theme-card.selected .theme-card__preview {
		border-color: var(--clr-core-pop-50);
	}

	.theme-card.selected .theme-card__label {
		background-color: var(--clr-scale-pop-80);
	}

	.theme-card.selected .theme-card__icon {
		opacity: 1;
		transform: scale(1);
	}
</style>
