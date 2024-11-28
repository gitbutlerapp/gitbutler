<script lang="ts">
	import MarkdownContent from './MarkdownContent.svelte';
	import { renderers } from '$lib/utils/markdownRenderers';
	import type { Tokens, Token } from 'marked';
	import type { Component } from 'svelte';

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
		| Tokens.List
		| Tokens.Strong
		| Tokens.Br;

	const { type, ...rest }: Props = $props();
</script>

{#if (!type || type === 'init') && 'tokens' in rest && rest.tokens}
	{#each rest.tokens as token}
		<!-- eslint-disable-next-line svelte/valid-compile -->
		<MarkdownContent {...token} />
	{/each}
{:else if renderers[type]}
	{@const CurrentComponent = renderers[type] as Component<Omit<Props, 'type'>>}
	{#if type === 'list'}
		{@const listItems = (rest as Extract<Props, { type: 'list' }>).items}
		<CurrentComponent {...rest}>
			{#each listItems as item}
				{@const ChildComponent = renderers[item.type]}
				<ChildComponent {...item}>
					<!-- eslint-disable-next-line svelte/valid-compile -->
					<MarkdownContent tokens={item.tokens} />
				</ChildComponent>
			{/each}
		</CurrentComponent>
	{:else}
		<CurrentComponent {...rest}>
			{#if 'tokens' in rest && rest.tokens}
				<!-- eslint-disable-next-line svelte/valid-compile -->
				<MarkdownContent tokens={rest.tokens} />
			{:else if 'raw' in rest}
				{rest.raw}
			{/if}
		</CurrentComponent>
	{/if}
{/if}
