<script lang="ts">
	import ScriptSwitcher from './ScriptSwitcher.svelte';
	import TerminalMockup from './TerminalMockup.svelte';
	import CtaButtons from '../components/CtaButtons.svelte';
	import scriptsData from '../scripts.json';
	import HeroHeader from '$home/sections/HeroHeader.svelte';
	import Header from '$lib/components/marketing/Header.svelte';

	import { type Snippet } from 'svelte';

	interface Props {
		currentPage?: 'home' | 'cli';
		descriptionContent: Snippet;
	}

	const { currentPage = 'home', descriptionContent }: Props = $props();

	let selectedScript = $state('parallel-branches');

	function handleScriptChange(scriptId: string) {
		selectedScript = scriptId;
	}
</script>

<section class="hero">
	<Header disableLogoLink />

	<div class="hero-content">
		<HeroHeader {currentPage} {descriptionContent} />

		<CtaButtons />

		<div class="terminal-with-switcher">
			<TerminalMockup />
			<div class="script-switcher">
				<ScriptSwitcher {scriptsData} onScriptChange={handleScriptChange} />
			</div>
		</div>
	</div>
</section>

<style lang="postcss">
	.hero {
		display: grid;
		grid-template-columns: subgrid;
		grid-column: full-start / full-end;
		flex-direction: column;
		background: var(--color-hero-background);
		color: var(--color-hero-text);
	}

	.hero-content {
		display: flex;
		grid-column: narrow-start / off-center;
		flex-direction: column;
		max-width: 800px;
		padding-top: 52px;
	}

	.terminal-with-switcher {
		display: flex;
		position: relative;
		margin-top: 32px;
	}

	.script-switcher {
		display: flex;
		position: absolute;
		bottom: 0;
		left: 50%;
		justify-content: center;
		width: calc(100% - 30px);
		transform: translateY(60%) translateX(-50%);
	}
</style>
