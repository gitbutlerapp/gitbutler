<script lang="ts">
	import ScriptSwitcher from "./ScriptSwitcher.svelte";
	import TerminalMockup from "./TerminalMockup.svelte";
	import CtaButtons from "../components/CtaButtons.svelte";
	import scriptsData from "../scripts.json";
	import HeroHeader from "$home/sections/HeroHeader.svelte";
	import Header from "$lib/components/marketing/Header.svelte";

	import { type Snippet } from "svelte";
	import type { ScriptStep } from "./terminal-types";

	interface Props {
		currentPage?: "home" | "cli";
		descriptionContent: Snippet;
	}

	const { currentPage = "home", descriptionContent }: Props = $props();

	const scriptKeys = Object.keys(scriptsData);
	let selectedScript = $state("stacked-branches");
	let scriptProgress = $state(0);

	function handleScriptChange(scriptId: string) {
		selectedScript = scriptId;
		scriptProgress = 0;
	}

	function handleProgress(progress: number) {
		scriptProgress = progress;
	}

	function handleScriptComplete() {
		const currentIndex = scriptKeys.indexOf(selectedScript);
		const nextIndex = (currentIndex + 1) % scriptKeys.length;

		// Switch to next script immediately (delay is handled in TerminalMockup)
		selectedScript = scriptKeys[nextIndex];
	}

	function getScript(): ScriptStep[] | undefined {
		return scriptsData[selectedScript as keyof typeof scriptsData]?.script as
			| ScriptStep[]
			| undefined;
	}
</script>

<section class="hero">
	<Header />

	<div class="hero-content">
		<HeroHeader {currentPage} {descriptionContent} />

		<CtaButtons />

		<div class="terminal-with-switcher">
			<TerminalMockup
				height="400px"
				script={getScript()}
				onComplete={handleScriptComplete}
				onProgress={handleProgress}
			/>
		</div>
	</div>

	<div class="script-switcher">
		<ScriptSwitcher
			{scriptsData}
			onScriptChange={handleScriptChange}
			{selectedScript}
			{scriptProgress}
		/>
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
		grid-column: narrow-start / narrow-end;
		flex-direction: column;
		/* max-width: 800px; */
		padding-top: 52px;
	}

	.terminal-with-switcher {
		display: flex;
		position: relative;
		flex-direction: column;
		margin-top: 32px;
	}

	.script-switcher {
		display: flex;
		grid-column: full-start / full-end;
		align-items: center;
		justify-content: center;
		margin-top: 16px;
	}

	@media (max-width: 700px) {
		.script-switcher {
			position: relative;
			left: -16px;
			width: calc(100% + 32px);
			margin-top: 8px;
			transform: none;

			/* hide scrollbar */
			&::-webkit-scrollbar {
				display: none;
			}
		}
	}
</style>
