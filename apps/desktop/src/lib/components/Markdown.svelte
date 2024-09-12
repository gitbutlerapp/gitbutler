<script lang="ts">
	import MarkdownContent from '$lib/components/MarkdownContent.svelte';
	import { options } from '$lib/utils/markdownRenderers';
	import { Lexer, type TokensList } from 'marked';

	interface Props {
		content: string | undefined;
	}

	let { content }: Props = $props();

	let tokens = $state<TokensList>();

	$effect(() => {
		if (content) {
			const lexer = new Lexer(options);
			tokens = lexer.lex(content);
		}
	});
</script>

<div class="markdown-content">
	{#if tokens}
		<MarkdownContent type="init" {tokens} />
	{/if}
</div>

<style>
	.markdown-content {
		display: inline;
	}
</style>
