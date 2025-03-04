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
		width: 100%;
		display: flex;
	}
	.code {
		width: 100%;
		border-radius: var(--radius-s);
		background-color: var(--clr-diff-line-bg);
		border: 1px solid var(--clr-border-2);
		overflow-x: scroll;
		padding: 4px 8px;
		font-family: var(--fontfamily-mono);
		box-sizing: border-box;
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
