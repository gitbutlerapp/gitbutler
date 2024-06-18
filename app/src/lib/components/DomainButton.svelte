<script lang="ts">
	import UpdateBaseButton from './UpdateBaseButton.svelte';
	import { getContextStore } from '$lib/utils/context';
	import { tooltip } from '$lib/utils/tooltip';
	import { BaseBranch } from '$lib/vbranches/types';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';

	export let href: string;
	export let domain: string;
	export let label: string;
	export let iconSrc: string;
	export let isNavCollapsed: boolean;

	const baseBranch = getContextStore(BaseBranch);

	$: selected = $page.url.href.includes(href);
</script>

<button
	use:tooltip={isNavCollapsed ? label : ''}
	on:mousedown={async () => await goto(href)}
	class="domain-button text-base-14 text-semibold"
	class:selected
>
	{#if domain === 'workspace'}
		<img class="icon" src={iconSrc} alt="" />

		{#if !isNavCollapsed}
			<span class="text-base-14 text-semibold" class:collapsed-txt={isNavCollapsed}>{label}</span>
			{#if ($baseBranch?.behind || 0) > 0 && !isNavCollapsed}
				<UpdateBaseButton />
			{/if}
		{/if}
	{:else}
		<slot />
	{/if}
</button>

<style lang="postcss">
	.domain-button {
		display: flex;
		align-items: center;
		gap: 10px;
		border-radius: var(--radius-m);
		padding: 10px;
		color: var(--clr-text-1);
		transition: background-color var(--transition-fast);
	}

	.icon {
		border-radius: var(--radius-s);
		height: 20px;
		width: 20px;
		flex-shrink: 0;
	}

	.domain-button:not(.selected):hover,
	.domain-button:not(.selected):focus {
		background-color: var(--clr-bg-1-muted);
	}

	.selected {
		background-color: var(--clr-bg-2);
	}
</style>
