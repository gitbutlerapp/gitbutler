<script lang="ts">
	/* eslint svelte/valid-compile: "off" */
	import { renderers, options } from '$lib/utils/markdownRenderers';
	import { Lexer, type Token, type Tokens } from 'marked';

	type Props = {
		content?: string;
		tokens?: Token[];
	} & (
		| Tokens.Link
		| Tokens.Heading
		| Tokens.Image
		| Tokens.Space
		| Tokens.Blockquote
		| Tokens.Code
		| Tokens.Codespan
		| Tokens.Text
	);

	let { content, type, tokens, ...rest }: Props = $props();

	const lexer = new Lexer(options);
	if (!tokens && content) {
		tokens = lexer.lex(content);
	}
</script>

{#if !type && tokens}
	{#each tokens as token}
		<svelte:self {...token} />
	{/each}
{:else if type && renderers[type as keyof typeof renderers]}
	<svelte:component this={renderers[type as keyof typeof renderers]} {...rest}>
		{#if tokens}
			<svelte:self {tokens} />
		{:else}
			{rest.raw}
		{/if}
	</svelte:component>
{:else if tokens}
	<svelte:self {tokens} />
{:else}
	{@html rest.raw?.replaceAll('\n', '<br />') ?? ''}
{/if}
