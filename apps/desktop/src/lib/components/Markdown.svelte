<script lang="ts">
	import { defaultRenderers, defaultOptions } from '$lib/utils/markdownRenderers';
	import { Lexer, type Token } from 'marked';

	interface Props {
		content?: string;
		type?: string;
		tokens?: Token[];
	}

	let { content, type, tokens, ...rest }: Props = $props();

	const lexer = new Lexer(defaultOptions);
	if (!tokens && content) {
		tokens = lexer.lex(content);
	}

	$inspect('TOKENS', tokens);
</script>

<div class="markdown-wrapper">
	{#if !type && tokens}
		{#each tokens as token}
			<svelte:self {...token} />
		{/each}
	{:else if type && defaultRenderers[type]}
		<svelte:component this={defaultRenderers[type]} {...rest}>
			{#if tokens}
				<svelte:self {tokens} />
			{/if}
		</svelte:component>
	{:else if tokens}
		<svelte:self {tokens} />
	{:else}
		{rest.text}
	{/if}
</div>

<style>
	.markdown-wrapper {
		display: inline;
	}
</style>
