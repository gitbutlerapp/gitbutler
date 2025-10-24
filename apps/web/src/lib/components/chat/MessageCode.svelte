<script lang="ts">
	import { codeContentToTokens, parserFromExtension } from '@gitbutler/ui/utils/diffParsing';

	interface Props {
		text: string;
		lang: string;
	}

	const { text, lang }: Props = $props();

	const parser = $derived(parserFromExtension(lang));
	const lines = $derived(codeContentToTokens(text, parser));
</script>

<div class="code-wrapper scrollbar">
	<code class="code">
		{#each lines as line, i (i)}
			<p class="line">
				{@html line.join('')}
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
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1);

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
