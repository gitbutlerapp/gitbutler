<script lang="ts">
	import Link from '$lib/link/Link.svelte';
	import Self from '$lib/markdown/MarkdownContent.svelte';
	import Blockquote from '$lib/markdown/markdownRenderers/Blockquote.svelte';
	import Br from '$lib/markdown/markdownRenderers/Br.svelte';
	import Code from '$lib/markdown/markdownRenderers/Code.svelte';
	import Codespan from '$lib/markdown/markdownRenderers/Codespan.svelte';
	import Em from '$lib/markdown/markdownRenderers/Em.svelte';
	import Heading from '$lib/markdown/markdownRenderers/Heading.svelte';
	import Html from '$lib/markdown/markdownRenderers/Html.svelte';
	import Image from '$lib/markdown/markdownRenderers/Image.svelte';
	import List from '$lib/markdown/markdownRenderers/List.svelte';
	import ListItem from '$lib/markdown/markdownRenderers/ListItem.svelte';
	import Paragraph from '$lib/markdown/markdownRenderers/Paragraph.svelte';
	import Strong from '$lib/markdown/markdownRenderers/Strong.svelte';
	import Text from '$lib/markdown/markdownRenderers/Text.svelte';
	import type { Token } from 'marked';
	import type { Component } from 'svelte';

	type Props = { type: 'init'; tokens: Token[] } | Token;

	const renderers = {
		link: Link,
		image: Image,
		blockquote: Blockquote,
		code: Code,
		codespan: Codespan,
		text: Text,
		html: Html,
		list: List,
		list_item: ListItem,
		heading: Heading,
		paragraph: Paragraph,
		init: null,
		br: Br,
		strong: Strong,
		em: Em
	};

	const { type, ...rest }: Props = $props();
</script>

{#if type === 'init' && 'tokens' in rest && rest.tokens}
	{#each rest.tokens as token}
		<Self {...token} />
	{/each}
{:else if renderers[type as keyof typeof renderers]}
	{@const CurrentComponent = renderers[type as keyof typeof renderers] as Component<
		Omit<Props, 'type'>
	>}
	{#if type === 'list'}
		{@const listItems = (rest as Extract<Props, { type: 'list' }>).items}
		<CurrentComponent {...rest}>
			{#each listItems as item}
				{@const ChildComponent = renderers[item.type]}
				<ChildComponent {...item}>
					<Self type="init" tokens={item.tokens} />
				</ChildComponent>
			{/each}
		</CurrentComponent>
	{:else}
		<CurrentComponent {...rest}>
			{#if 'tokens' in rest && rest.tokens}
				<Self type="init" tokens={rest.tokens} />
			{:else if 'raw' in rest}
				{rest.raw}
			{/if}
		</CurrentComponent>
	{/if}
{/if}
