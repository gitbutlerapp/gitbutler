<script lang="ts">
	import MarkdownContent from '$lib/components/MarkdownContent.svelte';
	import { options } from '$lib/utils/markdownRenderers';
	import { Lexer } from 'marked';

	interface Props {
		content: string | undefined;
	}

	let { content }: Props = $props();

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

<style>
	:global(.markdown p:last-child),
	:global(.markdown ul:last-child),
	:global(.markdown ol:last-child),
	:global(.markdown blockquote:last-child),
	:global(.markdown pre:last-child),
	:global(.markdown hr:last-child) {
		margin-bottom: 0;
	}

	:global(.markdown h1) {
		font-size: 2em;
		margin-bottom: 0.8em;
	}

	:global(.markdown h2) {
		font-size: 1.5em;
		margin-bottom: 0.8em;
	}

	:global(.markdown h3) {
		font-size: 1.17em;
		margin-bottom: 0.8em;
	}

	:global(.markdown h4) {
		font-size: 1em;
		margin-bottom: 0.8em;
	}

	:global(.markdown p) {
		margin-bottom: 1.2em;
	}

	:global(.markdown ul) {
		display: block;
		list-style-type: disc;
		margin: 1em 0;
		padding: 0 0 0 2em;
	}

	:global(.markdown ol) {
		display: block;
		list-style-type: decimal;
		margin: 1em 0;
		padding: 0 0 0 2em;
	}

	:global(.markdown li) {
		margin: 0.5em 0;
	}

	:global(.markdown blockquote) {
		margin: 1em 0;
		padding: 0 0 0 2em;
	}

	:global(.markdown pre) {
		margin: 1em 0;
		padding: 1em;
		background-color: var(--clr-scale-ntrl-90);
		border: 1px solid var(--clr-scale-ntrl-70);
		overflow: auto;
		border-radius: var(--radius-m);
	}

	:global(.markdown code) {
		font-family: monospace;
		background-color: var(--clr-scale-ntrl-90);
		border: 1px solid var(--clr-scale-ntrl-70);
		padding: 0 0.5em;
	}

	:global(.markdown a) {
		text-decoration: underline;
	}

	:global(.markdown hr) {
		box-sizing: content-box;
		height: 0;
		overflow: visible;
		margin: 2em 0;
		opacity: 0.2;
	}

	:global(.markdown b),
	:global(.markdown strong) {
		font-weight: bolder;
	}
</style>
