<script lang="ts">
	import MessageMarkdownContent from './MessageMarkdownContent.svelte';
	import { Lexer } from 'marked';
	import type { UserSimple } from '@gitbutler/shared/users/types';

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
		mentions: UserSimple[];
		content: string | undefined;
	}

	const { content, mentions }: Props = $props();

	const tokens = $derived.by(() => {
		const lexer = new Lexer(options);
		return lexer.lex(content ?? '');
	});
</script>

<div class="markdown">
	{#if tokens}
		<MessageMarkdownContent type="init" {tokens} {mentions} />
	{/if}
</div>
