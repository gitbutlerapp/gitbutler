<script lang="ts">
	import RangeControl from "./RangeControl.svelte";

	interface IllustrationColor {
		h: number;
		s: number;
		l: number;
	}

	interface Props {
		colors: Record<string, IllustrationColor>;
		onColorChange: (colorId: string, hsl: IllustrationColor) => void;
		isExpanded: boolean;
		onToggle: () => void;
	}

	let { colors = $bindable(), onColorChange, isExpanded, onToggle }: Props = $props();

	function getColorString(color: IllustrationColor): string {
		return `hsl(${color.h}, ${color.s}%, ${color.l}%)`;
	}

	function updateColor(colorId: string, prop: "h" | "s" | "l", value: number) {
		const updated = { ...colors[colorId], [prop]: value };
		onColorChange(colorId, updated);
	}

	// Return appropriate text color based on lightness only
	function getContrastColor(color: IllustrationColor): string {
		// Return black for light backgrounds, white for dark backgrounds
		// Using 50% lightness as the threshold
		return color.l > 50 ? "#000" : "#fff";
	}

	const colorLabels: Record<string, string> = {
		"art-scene-bg": "Art background",
		"art-scene-fill": "Fill",
		"art-scene-outline": "Outline",
	};

	const colorDescriptions: Record<string, string> = {
		"art-scene-bg": "Dark mode is separate. Switch themes to edit.",
	};
</script>

<div
	class="illustration-container"
	class:compact={!isExpanded}
	onclick={onToggle}
	role="presentation"
>
	{#each Object.entries(colors) as [colorId, color] (colorId)}
		{@const colorString = getColorString(color)}
		{@const contrastColor = getContrastColor(color)}
		<div
			class="color-block"
			style="background-color: {colorString}; --card-ui-color: {contrastColor}"
		>
			{#if isExpanded}
				<div class="controls" onclick={(e) => e.stopPropagation()} role="presentation">
					<div class="scale-header">
						<div class="stack-v gap-14">
							<div class="stack-v gap-4">
								<span class="text-15 text-body text-bold scale-name">{colorLabels[colorId]}</span>
								<span class="text-12 text-body op-60">{colorDescriptions[colorId] ?? " "}</span>
							</div>

							<RangeControl
								label="Hue"
								min={0}
								max={360}
								bind:value={color.h}
								oninput={() => updateColor(colorId, "h", color.h)}
								suffix="°"
							/>
							<RangeControl
								label="Saturation"
								min={0}
								max={100}
								bind:value={color.s}
								oninput={() => updateColor(colorId, "s", color.s)}
							/>
							<RangeControl
								label="Lightness"
								min={0}
								max={100}
								bind:value={color.l}
								oninput={() => updateColor(colorId, "l", color.l)}
							/>
						</div>
					</div>
				</div>
			{/if}
		</div>
	{/each}
</div>

<style>
	.illustration-container {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		min-height: 32px;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
	}

	.illustration-container:not(.compact) {
		flex: 1;
		min-height: 0;
	}

	.illustration-container.compact {
		height: 32px;
		cursor: pointer;
	}

	.color-block {
		display: flex;
		position: relative;
		align-items: center;
		justify-content: center;
		min-height: 32px;
		transition: background-color var(--transition-fast);
	}

	.controls {
		display: flex;
		flex-direction: column;
		width: 100%;
		height: 100%;
		cursor: default;
		pointer-events: auto;
	}

	.scale-header {
		display: flex;
		flex-direction: column;
		justify-content: space-between;
		height: 100%;
		padding: 24px 16px;
		gap: 12px;
	}

	.scale-name {
		color: var(--card-ui-color);
	}

	@media (max-width: 1024px) {
		.illustration-container {
			grid-template-columns: 1fr;
			border: none;
		}

		.color-block:not(:has(.controls)) {
			display: none;
		}
	}
</style>
