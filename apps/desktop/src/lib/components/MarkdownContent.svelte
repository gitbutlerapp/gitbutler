<script lang="ts">
	/* eslint svelte/valid-compile: "off" */
	import { renderers } from '$lib/utils/markdownRenderers';
	import type { TokensList, Tokens } from 'marked';

	type Props =
		| { type: 'init'; tokens: TokensList }
		| Tokens.Link
		| Tokens.Heading
		| Tokens.Image
		| Tokens.Space
		| Tokens.Blockquote
		| Tokens.Code
		| Tokens.Codespan
		| Tokens.Text;

	let { type, ...rest }: Props = $props();
</script>

{#if !type && rest.tokens}
	{#each rest.tokens as token}
		<svelte:self {...token} />
	{/each}
{:else if type && renderers[type as keyof typeof renderers]}
	<svelte:component this={renderers[type as keyof typeof renderers]} {...rest}>
		{#if rest.tokens}
			<svelte:self tokens={rest.tokens} />
		{:else}
			{rest.raw}
		{/if}
	</svelte:component>
{:else}
	{@html rest.raw?.replaceAll('\n', '<br />') ?? ''}
{/if}
