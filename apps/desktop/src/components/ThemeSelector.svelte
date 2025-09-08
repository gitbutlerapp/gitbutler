<script lang="ts">
	import { Icon } from '@gitbutler/ui';
	import type { Settings } from '$lib/settings/userSettings';
	import type { Writable } from 'svelte/store';

	interface Props {
		userSettings: Writable<Settings>;
	}

	const { userSettings }: Props = $props();

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
			name: 'Color blind friendly',
			value: 'color-blind',
			preview: '/images/theme-previews/color-blind.svg'
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
				onchange={() => userSettings.update((s) => ({ ...s, theme: theme.value }))}
			/>
			<div class="theme-card__preview">
				<i class="theme-card__icon"><Icon name="success" color="pop" /></i>

				<img src={theme.preview} alt={theme.name} />
			</div>

			<span class="theme-card__label text-12 text-semibold">{theme.name}</span>
		</label>
	{/each}
</fieldset>

<style lang="postcss">
	.cards-group {
		display: grid;
		grid-template-columns: repeat(2, 1fr);
		gap: 16px;
	}

	.theme-card {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 8px;
		cursor: pointer;
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
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);

		& img {
			width: 100%;
			height: auto;
			border-radius: var(--radius-m);
		}
	}

	.theme-card__label {
		padding: 6px;
		border-radius: var(--radius-m);
		text-align: center;
	}

	.theme-card__icon {
		z-index: 1;
		position: absolute;
		right: 6px;
		bottom: 6px;
		opacity: 0;
	}

	.hidden-input {
		z-index: -1;
		position: absolute;
		width: 0;
		height: 0;
	}

	/* MODIFIER */

	.theme-card.selected .theme-card__preview {
		border-color: var(--clr-core-pop-50);
	}

	.theme-card.selected .theme-card__label {
		background-color: var(--clr-scale-pop-80);
	}

	.theme-card.selected .theme-card__icon {
		transform: scale(1);
		opacity: 1;
	}
</style>
