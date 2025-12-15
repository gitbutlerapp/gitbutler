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
	import { generateScale } from './utils/colorScale';
	import { copyToClipboard } from './utils/export';
	import Header from '$lib/components/marketing/Header.svelte';
	import ThemeSwitcher from '$lib/components/marketing/ThemeSwitcher.svelte';

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

		<ExportSection {currentColors} />
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

<style>
	/* Layout */
	.container {
		display: grid;
		grid-template-rows: auto auto 1fr auto;
		grid-template-columns: subgrid;
		row-gap: 40px;
		grid-column: full-start / full-end;
		height: 100vh;
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
		grid-column: full-start / full-end;
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
		border-top-left-radius: var(--radius-ml);
		box-shadow: inset 0 1px 0 0 var(--clr-border-2);
	}

	.dotted-pattern {
		background-image: radial-gradient(
			oklch(from var(--clr-theme-gray-element) l c h / 0.1) 1px,
			#ffffff00 1px
		);

		background-size: 5px 5px;
	}

	/* Media Queries */
	@media (max-width: 1024px) {
		.app-mockup__header-center {
			margin-right: 0;
		}

		.app-mockup {
			display: none;
		}
	}
</style>
