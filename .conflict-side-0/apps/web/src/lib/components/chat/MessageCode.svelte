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

<div class="code-wrapper">
	<code class="code">
		{#each lines as line, i (i)}
			<p class="line">
				{@html line.join('')}
			</p>
		{/each}
	</code>
</div>

<style>
	.code-wrapper {
		display: flex;
		min-width: 0;
		max-width: 640px;
		overflow-x: scroll;
		border-radius: var(--radius-s);

		border: 1px solid var(--clr-border-2);
		padding: 4px 8px;
	}
	.code {
		width: 100%;
		max-width: 100%;
		font-family: var(--fontfamily-mono);
		box-sizing: border-box;
		background-color: var(--clr-diff-line-bg);
		border: none;
	}

	.line {
		width: 100%;
		white-space: pre;
		text-wrap: nowrap;
		tab-size: 4;
		cursor: text;
		line-height: normal;
		padding: 0;
		margin: 0;
	}
</style>
