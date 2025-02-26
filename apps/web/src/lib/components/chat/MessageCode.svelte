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

<div class="code">
	{#each lines as line, i (i)}
		<div class="line">
			{@html line.join('')}
		</div>
	{/each}
</div>

<style>
	.code {
		width: 100%;
		border-radius: var(--radius-s);
		background-color: var(--clr-diff-line-bg);
		border: 1px solid var(--clr-border-2);
		overflow-x: scroll;
		padding: 4px 8px;
		white-space: pre;
	}

	.line {
		width: 100%;
		white-space: pre;
		text-wrap: nowrap;
		tab-size: 4;
		cursor: text;
	}
</style>
