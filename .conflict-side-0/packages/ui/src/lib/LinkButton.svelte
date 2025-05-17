<script lang="ts">
	import Icon from '$lib/Icon.svelte';
	import { type Snippet } from 'svelte';
	import type iconsJson from '$lib/data/icons.json';

	interface Props {
		icon?: keyof typeof iconsJson;
		truncate?: boolean;
		onclick: (event?: any) => void;
		children: Snippet;
	}

	const { icon = 'open-link', truncate, onclick, children }: Props = $props();
</script>

<button
	type="button"
	class="link-button"
	class:link-button_truncate={truncate}
	onclick={(e) => {
		e.stopPropagation();
		onclick(e);
	}}
>
	<span class:truncate>
		{@render children()}
	</span>
	<div class="link-button__icon">
		{#if icon}
			<Icon name={icon} />
		{/if}
	</div>
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

	.link-button_truncate {
		overflow: hidden;
	}

	.link-button__icon {
		display: inline-flex;
		align-items: center;
		opacity: 0.8;
	}
</style>
