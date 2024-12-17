<script lang="ts">
	import MarkdownContent from '$lib/components/MarkdownContent.svelte';
	import { options } from '$lib/utils/markdownRenderers';
	import { Lexer } from 'marked';

	interface Props {
		content: string | undefined;
	}

	const { content }: Props = $props();

	const tokens = $derived.by(() => {
		const lexer = new Lexer(options);
		return lexer.lex(content ?? '');
	});
</script>

<div class="markdown">
	{#if tokens}
		<MarkdownContent type="init" {tokens} />
	{/if}
</div>
