<script lang="ts">
	import {
		clearHighlightingCaches,
		codeContentToTokens,
		langFromExtension,
	} from "@gitbutler/ui/utils/diffParsing";
	import { onHighlighterChange } from "@gitbutler/ui/utils/shikiHighlighter";

	interface Props {
		text: string;
		lang: string;
	}

	const { text, lang }: Props = $props();

	// Reactive trigger: re-derive when shiki highlighter becomes ready
	// or the app theme (light/dark) changes.
	let highlighterVersion = $state(0);
	$effect(() => {
		return onHighlighterChange(() => {
			clearHighlightingCaches();
			highlighterVersion += 1;
		});
	});

	const langId = $derived(langFromExtension(lang));
	const lines = $derived.by(() => {
		void highlighterVersion;
		return codeContentToTokens(text, langId);
	});
</script>

<div class="code-wrapper scrollbar">
	<code class="code">
		{#each lines as line, i (i)}
			<p class="line">
				{@html line.join("")}
			</p>
		{/each}
	</code>
</div>

<style lang="postcss">
	.code-wrapper {
		display: flex;
		min-width: 0;
		max-width: 640px;
		padding: 8px;
		overflow-x: auto;
		border: 1px solid var(--border-2);
		border-radius: var(--radius-m);
		background-color: var(--bg-1);

		@media (--tablet-viewport) {
			max-width: 80vw;
		}
	}
	.code {
		box-sizing: border-box;
		width: 100%;
		max-width: 100%;
		padding: 0;
		border: none;
		font-family: var(--font-mono);
	}

	.line {
		width: 100%;
		margin: 0;
		padding: 0;
		line-height: normal;
		text-wrap: nowrap;
		white-space: pre;
		cursor: text;
		tab-size: 4;
	}
</style>
