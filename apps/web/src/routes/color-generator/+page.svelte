<script lang="ts">
	import appHeaderCenterSvg from './assets/app-header-center.svg?raw';
	import appHeaderLeftSvg from './assets/app-header-left.svg?raw';
	import appHeaderRightSvg from './assets/app-header-right.svg?raw';
	import appLanesSvg from './assets/app-lanes.svg?raw';
	import appSidebarSvg from './assets/app-sidebar.svg?raw';
	import appUnassignedSvg from './assets/app-unassigned.svg?raw';
	import ColorScaleDisplay from './components/ColorScaleDisplay.svelte';
	import ExportSection from './components/ExportSection.svelte';
	import SemanticZones from './components/SemanticZones.svelte';

	import {
		SCALES,
		SHADES,
		DEFAULT_SATURATIONS,
		DEFAULT_SHADE_50_LIGHTNESS
	} from './constants/colorScales';
	import { hexToRgb, rgbToHsl } from './utils/colorConversion';
	import { generateScale } from './utils/colorScale';
	import { copyToClipboard } from './utils/export';
	import GitbutlerLogoLink from '$lib/components/GitbutlerLogoLink.svelte';

	let scaleSaturations: Record<string, number> = $state({
		...DEFAULT_SATURATIONS
	});
	let scaleHues: Record<string, number | null> = $state({
		ntrl: null,
		pop: null,
		err: null,
		warn: null,
		succ: null,
		purp: null
	});
	let scaleShade50Lightness: Record<string, number> = $state({
		...DEFAULT_SHADE_50_LIGHTNESS
	});

	let expandedScaleId: string | null = $state('pop');

	const baseHue = 180;

	// Generate colors reactively whenever dependencies change
	let currentColors = $derived(
		generateScale(SCALES, SHADES, scaleSaturations, scaleHues, baseHue, scaleShade50Lightness)
	);

	function updateScaleHue(scaleId: string, hexColor: string) {
		const rgb = hexToRgb(hexColor);
		if (!rgb) return;
		const hsl = rgbToHsl(rgb.r, rgb.g, rgb.b);
		scaleHues[scaleId] = hsl.h;
	}

	function updateScaleSaturation(scaleId: string, value: number) {
		scaleSaturations[scaleId] = value;
	}

	function updateScaleShade50Lightness(scaleId: string, value: number) {
		scaleShade50Lightness[scaleId] = value;
	}

	function copyScaleJSON(scaleId: string) {
		const json = JSON.stringify(currentColors[scaleId], null, 2);
		copyToClipboard(json);
	}

	function toggleScale(scaleId: string) {
		expandedScaleId = expandedScaleId === scaleId ? null : scaleId;
	}
</script>

<svelte:head>
	<title>GitButler | Color Generator</title>
</svelte:head>

<div class="container">
	<div class="logo-column">
		<div class="logo">
			<GitbutlerLogoLink markOnly />
			<span class="logo-slash">/</span>
		</div>
	</div>

	<div class="content-column">
		<div class="content-header">
			<div class="stack-v gap-16">
				<h1 class="title">Nothing <i>But</i> Colors</h1>
				<p class="text-14 text-body clr-text-2">
					HSL-based color scale generator for the GitButler app.
					<br />
					Simply copy the generated colors or export them as JSON.
				</p>
			</div>

			<div class="divider">
				<ExportSection {currentColors} />
			</div>
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
					onCopyJSON={copyScaleJSON}
				/>
			{/each}
		</div>

		<div class="app-mockup-wrapper">
			<div class="app-mockup">
				<div class="app-mockup__header">
					{@html appHeaderLeftSvg}
					<div class="app-mockup__header-center">
						{@html appHeaderCenterSvg}
					</div>
					{@html appHeaderRightSvg}
				</div>
				<div class="app-mockup__body">
					<div class="app-mockup__sidebar">
						{@html appSidebarSvg}
					</div>
					<div class="app-mockup__unassigned">
						{@html appUnassignedSvg}
					</div>
					<div class="app-mockup__lanes dotted-pattern">
						{@html appLanesSvg}
					</div>
				</div>
			</div>
		</div>
	</div>
</div>

<style>
	.app-mockup-wrapper {
		position: relative;
	}

	.logo {
		display: flex;
		align-items: center;
		gap: 18px;
	}

	.logo-slash {
		font-size: 40px;
		font-family: var(--font-accent);
		opacity: 0.2;
	}

	.app-mockup {
		display: flex;
		flex-direction: column;
		overflow: hidden;
		transform: translateY(1px);
		border: 1px solid var(--clr-border-2);
		border-bottom: none;
		border-radius: 16px 16px 0 0;
		background-color: var(--clr-bg-2);
		box-shadow: 0 14px 36px rgba(0, 0, 0, 0.2);
	}

	.app-mockup__header {
		display: flex;
		justify-content: space-between;
		width: 100%;
		padding: 14px;
		gap: 8px;
	}

	.app-mockup__header-center {
		display: flex;
		justify-content: center;
		margin-right: 14%;
	}

	.app-mockup__body {
		display: flex;
		flex-shrink: 0;
		width: 100%;
	}

	.app-mockup__sidebar {
		margin-right: 16px;
	}

	.app-mockup__lanes {
		display: flex;
		flex-grow: 1;
		flex-shrink: 0;
		margin-right: -1px;
		margin-left: 8px;
		overflow: hidden;
		overflow: hidden;
		border-top-left-radius: var(--radius-ml);
		box-shadow: inset 0 1px 0 0 var(--clr-border-2);
	}

	.dotted-pattern {
		background-image: radial-gradient(
			oklch(from var(--clr-scale-ntrl-50) l c h / 0.13) 1px,
			#ffffff00 1px
		);
		background-size: 5px 5px;
	}

	.container {
		display: grid;
		grid-template-columns: subgrid;
		row-gap: 30px;
		grid-column: full-start / full-end;
		min-height: 100vh;
		padding-top: 40px;
	}

	.title {
		font-size: 60px;
		line-height: 1;
		font-family: var(--font-accent);
		letter-spacing: -1px;
	}

	.logo-column {
		display: flex;
		grid-column: 1 / 1;
		flex-direction: column;
		align-items: center;
	}

	.content-header {
		display: flex;
		justify-content: space-between;
		gap: 24px;
	}

	.content-column {
		display: flex;
		grid-column: 2 / 13;
		flex-direction: column;
		height: 100%;
		gap: 40px;
	}

	.scales-section {
		display: grid;

		/* 13 columns for color scales (10 shades, with shade 50 spanning 3 columns) */
		grid-template-columns: repeat(13, 1fr);
		row-gap: 12px;
		flex: 1;
	}
</style>
