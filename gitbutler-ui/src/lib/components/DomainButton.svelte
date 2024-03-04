<script lang="ts">
	import UpdateBaseButton from './UpdateBaseButton.svelte';
	import { tooltip } from '$lib/utils/tooltip';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { BaseBranchService } from '$lib/vbranches/branchStoresCache';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';

	export let href: string;
	export let domain: string;
	export let label: string;
	export let iconSrc: string;
	export let branchController: BranchController;
	export let baseBranchService: BaseBranchService;
	export let isNavCollapsed: boolean;

	$: base$ = baseBranchService.base$;

	$: selected = $page.url.href.includes(href);
</script>

<button
	use:tooltip={isNavCollapsed ? label : ''}
	on:mousedown={() => goto(href)}
	class="domain-button text-base-14 text-semibold"
	class:selected
>
	{#if domain === 'workspace'}
		<img class="icon" src={iconSrc} alt="" />

		{#if !isNavCollapsed}
			<span class="text-base-14 text-semibold" class:collapsed-txt={isNavCollapsed}>{label}</span>
			{#if ($base$?.behind || 0) > 0 && !isNavCollapsed}
				<UpdateBaseButton {branchController} />
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
		gap: var(--space-10);
		border-radius: var(--radius-m);
		padding: var(--space-10);
		color: var(--clr-theme-scale-ntrl-0);
		transition: background-color var(--transition-fast);
	}

	.icon {
		border-radius: var(--radius-s);
		height: var(--space-20);
		width: var(--space-20);
		flex-shrink: 0;
	}

	.domain-button:not(.selected):hover,
	.domain-button:not(.selected):focus,
	.selected {
		background-color: color-mix(in srgb, transparent, var(--darken-tint-light));
	}
</style>
