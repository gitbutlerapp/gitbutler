<script lang="ts">
	import { renderers, options } from '$lib/utils/markdownRenderers';
	import { Lexer, type Tokens } from 'marked';

	interface Props {
		content?: string;
	}

	let { content, type, tokens, ...rest }: Props & Tokens.Generic = $props();

	const lexer = new Lexer(options);
	if (!tokens && content) {
		tokens = lexer.lex(content);
	}

	$inspect('TOKENS', { tokens, rest });
</script>

{#if !type && tokens}
	{#each tokens as token}
		<svelte:self {...token} />
	{/each}
{:else if type && renderers[type]}
	<svelte:component this={renderers[type]} {...rest} {options}>
		{#if tokens}
			<svelte:self {tokens} />
		{:else}
			{rest.raw}
		{/if}
	</svelte:component>
{:else if tokens}
	<svelte:self {tokens} />
{:else if rest.items}
	{#each rest.items as item}
		{@html item.raw}<br />
	{/each}
{:else}
	{rest.raw}
{/if}
