<script lang="ts">
	import ColorCard from './ColorCard.svelte';
	import ScaleControls from './ScaleControls.svelte';
	import type { ColorScale, Shade } from '../types/color';

	interface Props {
		scale: ColorScale;
		shades: Shade[];
		colors: Record<number, string>;
		saturation: number;
		shade50Lightness: number;
		hue: number | null;
		isExpanded: boolean;
		onToggle: (scaleId: string) => void;
		onHueChange: (scaleId: string, hue: number) => void;
		onSaturationChange: (scaleId: string, value: number) => void;
		onShade50LightnessChange: (scaleId: string, value: number) => void;
		onCopyJSON: (scaleId: string) => void;
	}

	let {
		scale,
		shades,
		colors,
		saturation = $bindable(),
		shade50Lightness = $bindable(),
		hue,
		isExpanded,
		onToggle,
		onHueChange,
		onSaturationChange,
		onShade50LightnessChange,
		onCopyJSON
	}: Props = $props();

	function toggleExpanded() {
		if (!isExpanded) {
			onToggle(scale.id);
		}
	}
</script>

<div
	class="scale-container"
	class:compact={!isExpanded}
	onclick={toggleExpanded}
	role="presentation"
>
	{#each shades.filter((s) => s.value !== 100 && s.value !== 0) as shade (shade.value)}
		{@const color = colors[shade.value] || '#000'}
		<ColorCard {shade} {color} scaleId={scale.id}>
			{#if shade.value === 50 && isExpanded}
				<div class="scale-controls" onclick={(e) => e.stopPropagation()} role="presentation">
					<ScaleControls
						{scale}
						bind:saturation
						bind:shade50Lightness
						{hue}
						{onHueChange}
						{onSaturationChange}
						{onShade50LightnessChange}
						{onCopyJSON}
					/>
				</div>
			{/if}
		</ColorCard>
	{/each}
</div>

<style>
	.scale-container {
		display: grid;
		/* 13 columns for color scales (10 shades, with shade 50 spanning 3 columns) */
		grid-template-columns: repeat(13, 1fr);
		min-height: 32px;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		transition: height var(--transition-medium);
	}

	.scale-container:not(.compact) {
		flex: 1;
		min-height: 0;
	}

	.scale-container.compact {
		height: 32px;
		overflow: hidden;
		cursor: pointer;
	}

	.scale-container :global(.color-card[data-shade='50']) {
		grid-column: span 3;
	}

	.scale-controls {
		display: flex;
		flex-direction: column;
		height: 100%;
		cursor: default;
		pointer-events: auto;
	}

	@media (max-width: 1024px) {
		.scale-container {
			grid-template-columns: repeat(1, 1fr);
			border: none;
		}

		.scale-container :global(.color-card[data-shade='50']) {
			grid-column: 1 / -1;
		}

		.scale-container :global(.color-card:not([data-shade='50'])) {
			display: none;
		}
	}
</style>
