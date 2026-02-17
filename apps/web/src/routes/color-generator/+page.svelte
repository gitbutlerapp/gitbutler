<script lang="ts">
	import appPreviewSvg from "./assets/app-preview.svg?raw";
	import artPreviewSvg from "./assets/art-preview.svg?raw";
	import ColorScaleDisplay from "./components/ColorScaleDisplay.svelte";
	import ExportSection from "./components/ExportSection.svelte";
	import IllustrationColors from "./components/IllustrationColors.svelte";
	import SemanticZones from "./components/SemanticZones.svelte";

	import {
		SCALES,
		SHADES,
		DEFAULT_SATURATIONS,
		DEFAULT_SHADE_50_LIGHTNESS,
		ART_COLORS_LIGHT,
		ART_COLORS_DARK,
	} from "./constants/colorScales";
	import { generateScale } from "./utils/colorScale";
	import Header from "$lib/components/marketing/Header.svelte";
	import ThemeSwitcher from "$lib/components/marketing/ThemeSwitcher.svelte";
	import { effectiveThemeStore } from "$lib/utils/theme.svelte";

	let scaleSaturations: Record<string, number> = $state({
		...DEFAULT_SATURATIONS,
	});
	let scaleHues: Record<string, number | null> = $state({
		gray: null,
		pop: null,
		err: null,
		warn: null,
		succ: null,
		purp: null,
	});
	let scaleShade50Lightness: Record<string, number> = $state({
		...DEFAULT_SHADE_50_LIGHTNESS,
	});

	let artColorOverridesLight: Record<string, { h: number; s: number; l: number }> = $state({});
	let artColorOverridesDark: Record<string, { h: number; s: number; l: number }> = $state({});

	let illustrationColors = $derived({
		...($effectiveThemeStore === "dark" ? ART_COLORS_DARK : ART_COLORS_LIGHT),
		...($effectiveThemeStore === "dark" ? artColorOverridesDark : artColorOverridesLight),
	});

	let currentArtColorsLight = $derived({ ...ART_COLORS_LIGHT, ...artColorOverridesLight });
	let currentArtColorsDark = $derived({ ...ART_COLORS_DARK, ...artColorOverridesDark });

	let expandedScaleId: string | null = $state("pop");

	const baseHue = 180;

	// Generate colors reactively whenever dependencies change
	let currentColors = $derived(
		generateScale(SCALES, SHADES, scaleSaturations, scaleHues, baseHue, scaleShade50Lightness),
	);

	// Apply CSS variables to document root
	$effect(() => {
		const root = document.documentElement;
		SCALES.forEach((scale) => {
			SHADES.forEach((shade) => {
				const value = currentColors[scale.id]?.[shade.value];
				if (value) {
					root.style.setProperty(`--clr-core-${scale.id}-${shade.value}`, value);
				}
			});
		});
		// Apply illustration colors
		Object.entries(illustrationColors).forEach(([colorId, color]) => {
			const colorString = `hsl(${color.h}, ${color.s}%, ${color.l}%)`;
			root.style.setProperty(`--clr-${colorId}`, colorString);
		});
	});

	function updateScaleHue(scaleId: string, hue: number) {
		scaleHues[scaleId] = hue;
	}

	function updateScaleSaturation(scaleId: string, value: number) {
		scaleSaturations[scaleId] = value;
	}

	function updateScaleShade50Lightness(scaleId: string, value: number) {
		scaleShade50Lightness[scaleId] = value;
	}

	function toggleScale(scaleId: string) {
		expandedScaleId = expandedScaleId === scaleId ? null : scaleId;
	}

	function updateArtColor(colorId: string, hsl: { h: number; s: number; l: number }) {
		if ($effectiveThemeStore === "dark") {
			artColorOverridesDark[colorId] = hsl;
		} else {
			artColorOverridesLight[colorId] = hsl;
		}
	}
</script>

<svelte:head>
	<title>GitButler | Color Generator</title>
</svelte:head>

<div class="container">
	<Header />

	<div class="about-section">
		<div class="about-header">
			<h1 class="title">Nothing <i>But</i> Colors</h1>
			<ThemeSwitcher />
		</div>
		<p class="text-14 text-body clr-text-2">
			HSL-based color scale generator for the GitButler app.
			<br />
			Copy CSS and replace it in the app settings.
			<a class="underline" href="https://docs.gitbutler.com/color-theme" target="_blank">
				Read docs</a
			> ↗ to learn more.
		</p>

		<ExportSection
			{currentColors}
			artColorsLight={currentArtColorsLight}
			artColorsDark={currentArtColorsDark}
		/>
	</div>

	<div class="scales-section">
		<SemanticZones />
		{#each SCALES as scale (scale.id)}
			<ColorScaleDisplay
				{scale}
				shades={SHADES}
				colors={currentColors[scale.id] || {}}
				bind:saturation={scaleSaturations[scale.id]}
				bind:shade50Lightness={scaleShade50Lightness[scale.id]}
				hue={scaleHues[scale.id]}
				isExpanded={expandedScaleId === scale.id}
				onToggle={toggleScale}
				onHueChange={updateScaleHue}
				onSaturationChange={updateScaleSaturation}
				onShade50LightnessChange={updateScaleShade50Lightness}
			/>
		{/each}

		<IllustrationColors
			colors={illustrationColors}
			onColorChange={updateArtColor}
			isExpanded={expandedScaleId === "art"}
			onToggle={() => toggleScale("art")}
		/>
	</div>

	<div class="app-mockup-wrapper">
		<div class="app-mockup">
			{#if expandedScaleId === "art"}
				{@html artPreviewSvg}
			{:else}
				{@html appPreviewSvg}
			{/if}
		</div>
	</div>
</div>

<style>
	/* Layout */
	.container {
		display: grid;
		grid-template-rows: auto auto 1fr auto;
		grid-template-columns: subgrid;
		row-gap: 40px;
		grid-column: full-start / full-end;
		min-height: 100vh;
	}

	.about-section {
		display: flex;
		grid-column: narrow-start / narrow-end;
		flex-direction: column;
		gap: 16px;
	}

	.about-header {
		display: flex;
		gap: 16px;
	}

	.title {
		font-size: 60px;
		line-height: 1;
		font-family: var(--font-accent);
		letter-spacing: -1px;
	}

	.scales-section {
		display: flex;
		grid-column: full-start / full-end;
		flex-direction: column;
		gap: 12px;
	}

	/* App Mockup */
	.app-mockup-wrapper {
		position: relative;
		grid-column: narrow-start / narrow-end;
	}

	.app-mockup {
		display: flex;
		flex-direction: column;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-bottom: none;
		border-radius: 16px 16px 0 0;
		background-color: var(--clr-bg-2);
		box-shadow: 0 14px 36px rgba(0, 0, 0, 0.2);
	}

	/* Media Queries */
	@media (max-width: 1024px) {
		.app-mockup {
			display: none;
		}
	}
</style>
