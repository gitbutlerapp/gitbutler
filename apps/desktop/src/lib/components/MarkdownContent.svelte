<script lang="ts">
	/* eslint svelte/valid-compile: "off" */
	import { renderers } from '$lib/utils/markdownRenderers';
	import type { Tokens, Token } from 'marked';

	type Props =
		| { type: 'init'; tokens: Token[] }
		| Tokens.Link
		| Tokens.List
		| Tokens.ListItem
		| Tokens.Heading
		| Tokens.Image
		| Tokens.Space
		| Tokens.Blockquote
		| Tokens.Code
		| Tokens.Codespan
		| Tokens.Text;

	let { type, ...rest }: Props = $props();
</script>

{#if type && renderers[type as keyof typeof renderers]}
	{#if type === 'list'}
		<svelte:component this={renderers.list} {...rest}>
			{#each rest.items as item}
				<svelte:component this={renderers.orderedlistitem || renderers.listitem} {...item}>
					<svelte:self tokens={item.tokens} {renderers} />
				</svelte:component>
			{/each}
		</svelte:component>
	{:else}
		<svelte:component this={renderers[type as keyof typeof renderers] as any} {...rest}>
			{#if 'tokens' in rest}
				<svelte:self tokens={rest.tokens} />
			{/if}
		</svelte:component>
	{/if}
{:else if 'tokens' in rest && rest.tokens}
	{#each rest.tokens as token}
		<svelte:self {...token} />
	{/each}
{:else if 'raw' in rest}
	{@html rest.raw?.replaceAll('\n', '') ?? ''}
{/if}
