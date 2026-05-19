<script lang="ts">
	import { Icon } from "@gitbutler/ui";
	import { THEME_OPTIONS, type AppTheme } from "$lib/theme/themes";
	import type { UiState } from "$lib/state/uiState.svelte";

	interface Props {
		uiState: UiState;
	}

	const { uiState }: Props = $props();

	const currentTheme = $derived(uiState.global.theme.current ?? "system");
</script>

<fieldset class="cards-group">
	{#each THEME_OPTIONS as theme}
		<label class="theme-card" class:selected={theme.id === currentTheme} for="theme-{theme.id}">
			<input
				class="hidden-input"
				type="radio"
				id="theme-{theme.id}"
				value={theme.id}
				checked={theme.id === currentTheme}
				onchange={() => uiState.global.theme.set(theme.id as AppTheme)}
			/>
			<div
				class="theme-card__preview"
				style:--preview-page={theme.preview.page}
				style:--preview-sidebar={theme.preview.sidebar}
				style:--preview-panel={theme.preview.panel}
				style:--preview-element={theme.preview.element}
				style:--preview-accent={theme.preview.accent}
				style:--preview-text={theme.preview.text}
			>
				<i class="theme-card__icon"><Icon name="tick-circle" color={theme.preview.accent} /></i>

				<div class="theme-preview" aria-hidden="true">
					<div class="theme-preview__sidebar">
						<div class="theme-preview__pill"></div>
						<div class="theme-preview__line"></div>
						<div class="theme-preview__line theme-preview__line--short"></div>
					</div>
					<div class="theme-preview__main">
						<div class="theme-preview__window">
							<div class="theme-preview__toolbar"></div>
							<div class="theme-preview__card theme-preview__card--accent"></div>
							<div class="theme-preview__card"></div>
							<div class="theme-preview__card theme-preview__card--wide"></div>
						</div>
						<div class="theme-preview__rail">
							<div class="theme-preview__rail-line"></div>
							<div class="theme-preview__rail-panel"></div>
						</div>
					</div>
				</div>
			</div>

			<span class="theme-card__label text-12 text-semibold">{theme.label}</span>
		</label>
	{/each}
</fieldset>

<style lang="postcss">
	.cards-group {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
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
			background-color: var(--bg-2);
		}
	}

	.theme-card__preview {
		position: relative;
		width: 100%;
		height: 124px;
		overflow: hidden;
		border: 1px solid var(--border-2);
		border-radius: var(--radius-m);
	}

	.theme-card__label {
		padding: 6px;
		border-radius: var(--radius-m);
		text-align: center;
	}

	.theme-card__icon {
		display: flex;
		z-index: 1;
		position: absolute;
		right: 8px;
		bottom: 8px;
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
		border-color: var(--fill-pop-bg);
	}

	.theme-card.selected .theme-card__label {
		background-color: var(--chip-pop-bg);
	}

	.theme-card.selected .theme-card__icon {
		transform: scale(1);
		opacity: 1;
	}

	.theme-preview {
		display: grid;
		grid-template-columns: 42px 1fr;
		height: 100%;
		background: var(--preview-page);
	}

	.theme-preview__sidebar {
		display: flex;
		flex-direction: column;
		padding: 8px 6px;
		gap: 8px;
		border-right: 1px solid color-mix(in srgb, var(--preview-text) 16%, transparent);
		background: var(--preview-sidebar);
	}

	.theme-preview__pill,
	.theme-preview__line,
	.theme-preview__rail-line,
	.theme-preview__toolbar,
	.theme-preview__card,
	.theme-preview__rail-panel {
		border-radius: 999px;
		background: color-mix(in srgb, var(--preview-text) 18%, var(--preview-panel));
	}

	.theme-preview__pill {
		width: 24px;
		height: 12px;
		background: var(--preview-accent);
	}

	.theme-preview__line {
		width: 24px;
		height: 6px;
	}

	.theme-preview__line--short {
		width: 18px;
	}

	.theme-preview__main {
		display: grid;
		grid-template-columns: 1fr 40px;
		padding: 8px;
		gap: 10px;
	}

	.theme-preview__window,
	.theme-preview__rail {
		display: flex;
		flex-direction: column;
		padding: 6px;
		gap: 6px;
		border-radius: var(--radius-m);
		background: var(--preview-panel);
	}

	.theme-preview__toolbar {
		width: 100%;
		height: 11px;
	}

	.theme-preview__card {
		width: 100%;
		height: 10px;
		border-radius: var(--radius-s);
		background: var(--preview-element);
	}

	.theme-preview__card--accent {
		background: var(--preview-accent);
	}

	.theme-preview__card--wide {
		flex-grow: 1;
		min-height: 38px;
	}

	.theme-preview__rail-line {
		height: 8px;
	}

	.theme-preview__rail-panel {
		flex-grow: 1;
		border-radius: var(--radius-s);
		background: var(--preview-element);
	}
</style>
