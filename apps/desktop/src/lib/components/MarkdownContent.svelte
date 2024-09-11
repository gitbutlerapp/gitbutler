<script lang="ts">
	/* eslint svelte/valid-compile: "off" */
	import { renderers } from '$lib/utils/markdownRenderers';
	import type { Tokens, Token } from 'marked';

	type Props =
		| { type: 'init'; tokens: Token[] }
		| Tokens.Link
		| Tokens.Heading
		| Tokens.Image
		| Tokens.Blockquote
		| Tokens.Code
		| Tokens.Text
		| Tokens.Codespan
		| Tokens.Paragraph
		| Tokens.ListItem
		| Tokens.List;

	let { type, ...rest }: Props = $props();

	// @ts-expect-error todo: map cannot be indexed on a union of string literals apparently
	const CurrentComponent = renderers[type];
</script>

{#if (!type || type === 'init') && 'tokens' in rest && rest.tokens}
	{#each rest.tokens as token}
		<svelte:self {...token} />
	{/each}
{:else if renderers[type as Extract<Props, 'type'>]}
	{#if type === 'list'}
		{@const listItems = (rest as Extract<Props, { type: typeof type }>).items}
		<CurrentComponent {...rest}>
			{#each listItems as item}
				{@const ChildComponent = renderers[item.type]}
				<ChildComponent {...item}>
					<svelte:self tokens={item.tokens} />
				</ChildComponent>
			{/each}
		</CurrentComponent>
	{:else}
		<CurrentComponent this={renderers[type as Extract<Props, 'type'>]} {...rest}>
			{#if 'tokens' in rest && rest.tokens}
				<svelte:self tokens={rest.tokens} />
			{:else if 'raw' in rest}
				{rest.raw}
			{/if}
		</CurrentComponent>
	{/if}
{/if}
