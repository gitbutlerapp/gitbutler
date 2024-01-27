<script lang="ts">
	import { page } from '$app/stores';
	import Icon from '$lib/icons/Icon.svelte';
	import type { Branch } from '$lib/vbranches/types';
	import { slide } from 'svelte/transition';

	export let branch: Branch;
	export let projectId: string;

	$: href = `/${projectId}/stashed/${branch.id}`;
	$: selected = $page.url.href.includes(href);
</script>

<a class="item" {href} class:selected transition:slide={{ duration: 250 }}>
	<Icon name="branch" />
	<div class="text-color-2 flex-grow truncate">
		{branch.name}
		{branch.files[0]?.modifiedAt}
	</div>
</a>

<style lang="postcss">
	.item {
		display: flex;
		gap: var(--space-10);
		padding-top: var(--space-10);
		padding-bottom: var(--space-10);
		padding-left: var(--space-8);
		padding-right: var(--space-8);
		border-radius: var(--radius-m);
	}
	.item:hover,
	.item:focus,
	.selected {
		background-color: var(--clr-theme-container-pale);
	}
</style>
