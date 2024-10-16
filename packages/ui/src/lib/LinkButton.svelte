<script lang="ts">
	import Icon from '$lib/Icon.svelte';
	import { type Snippet } from 'svelte';
	import type iconsJson from '$lib/data/icons.json';

	interface Props {
		icon?: keyof typeof iconsJson;
		onclick: (event?: any) => void;
		children: Snippet;
	}

	const { icon = 'open-link', onclick, children }: Props = $props();
</script>

<button
	class="link-button"
	onclick={(e) => {
		e.stopPropagation();
		onclick(e);
	}}
>
	{@render children()}
	{#if icon}
		<Icon name={icon} />
	{/if}
</button>

<style lang="postcss">
	.link-button {
		cursor: pointer;
		display: inline-flex;
		align-items: center;
		gap: 2px;
		text-decoration: underline;
		user-select: text;

		&:hover {
			text-decoration: none;
		}
	}
</style>
