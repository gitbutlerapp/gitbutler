<script lang="ts">
	import Self from '$components/MarkdownContent.svelte';
	import { renderers } from '$lib/utils/markdownRenderers';
	import type { Token } from 'marked';
	import type { Component } from 'svelte';

	type Props = { type: 'init'; tokens: Token[] } | Token;

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
