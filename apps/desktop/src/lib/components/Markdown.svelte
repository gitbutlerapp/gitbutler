<script lang="ts">
	import MarkdownContent from '$lib/components/MarkdownContent.svelte';
	import { options } from '$lib/utils/markdownRenderers';
	import { Lexer } from 'marked';

	interface Props {
		content: string;
	}

	let { content }: Props = $props();

	const lexer = new Lexer(options);
	const tokens = lexer.lex(content);
</script>

<div class="markdown-content">
	<MarkdownContent type="init" {tokens} />
</div>

<style>
	.markdown-content {
		display: inline;
	}
</style>
