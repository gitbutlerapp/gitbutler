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

	// @ts-expect-error indexing on string union is having trouble
	const CurrentComponent = renderers[type as Props['type']];
</script>

{#if type && CurrentComponent}
	{#if type === 'list'}
		{@const listItems = (rest as Extract<Props, { type: typeof type }>).items}
		<CurrentComponent {...rest}>
			{#each listItems as item}
				{@const ChildComponent = renderers[item.type]}
				<ChildComponent {...item}>
					<svelte:self tokens={item.tokens} {renderers} />
				</ChildComponent>
			{/each}
		</CurrentComponent>
	{:else}
		<CurrentComponent {...rest}>
			{#if 'tokens' in rest}
				<svelte:self tokens={rest.tokens} />
			{/if}
		</CurrentComponent>
	{/if}
{:else if 'tokens' in rest && rest.tokens}
	{#each rest.tokens as token}
		<svelte:self {...token} />
	{/each}
{:else if 'raw' in rest}
	{@html rest.raw?.replaceAll('\n', '') ?? ''}
{/if}
