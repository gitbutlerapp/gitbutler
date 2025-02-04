<script lang="ts">
	import MessageMarkdownContent from './MessageMarkdownContent.svelte';
	import { Lexer } from 'marked';

	const options = {
		async: false,
		breaks: true,
		gfm: true,
		pedantic: false,
		renderer: null,
		silent: false,
		tokenizer: null,
		walkTokens: null
	};

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
		<MessageMarkdownContent type="init" {tokens} />
	{/if}
</div>
