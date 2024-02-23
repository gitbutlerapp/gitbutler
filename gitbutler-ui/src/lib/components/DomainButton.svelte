<script lang="ts">
	import { page } from '$app/stores';
	import type { Persisted } from '$lib/persisted/persisted';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { BaseBranchService } from '$lib/vbranches/branchStoresCache';
	import UpdateBaseButton from './UpdateBaseButton.svelte';

	export let href: string;
	export let domain: string;
	export let branchController: BranchController;
	export let baseBranchService: BaseBranchService;
	export let isNavCollapsed: Persisted<boolean>;

	$: base$ = baseBranchService.base$;

	$: selected = $page.url.href.includes(href);
</script>

<a
	{href}
	class="domain-button text-base-14 text-semibold"
	class:collapsed={$isNavCollapsed}
	class:selected
>
	{#if domain === 'workspace'}
		<div class="icon">
			<svg
				width="16"
				height="16"
				viewBox="0 0 16 16"
				fill="none"
				xmlns="http://www.w3.org/2000/svg"
			>
				<path
					d="M0 6.64C0 4.17295 0 2.93942 0.525474 2.01817C0.880399 1.39592 1.39592 0.880399 2.01817 0.525474C2.93942 0 4.17295 0 6.64 0H9.36C11.8271 0 13.0606 0 13.9818 0.525474C14.6041 0.880399 15.1196 1.39592 15.4745 2.01817C16 2.93942 16 4.17295 16 6.64V9.36C16 11.8271 16 13.0606 15.4745 13.9818C15.1196 14.6041 14.6041 15.1196 13.9818 15.4745C13.0606 16 11.8271 16 9.36 16H6.64C4.17295 16 2.93942 16 2.01817 15.4745C1.39592 15.1196 0.880399 14.6041 0.525474 13.9818C0 13.0606 0 11.8271 0 9.36V6.64Z"
					fill="#48B0AA"
				/>
				<rect x="2" y="3" width="6" height="10" rx="2" fill="#D9F3F2" />
				<rect opacity="0.7" x="10" y="3" width="4" height="10" rx="2" fill="#D9F3F2" />
			</svg>
		</div>

		<span class="text-base-13 text-semibold" class:collapsed-txt={$isNavCollapsed}>
			Workspace
		</span>
		{#if ($base$?.behind || 0) > 0 && !$isNavCollapsed}
			<UpdateBaseButton {branchController} />
		{/if}
	{:else}
		<slot />
	{/if}
</a>

<style lang="postcss">
	.domain-button {
		display: flex;
		align-items: center;
		gap: var(--space-10);
		border-radius: var(--radius-m);
		padding: var(--space-10) var(--space-8);
		color: var(--clr-theme-scale-ntrl-0);
		height: var(--space-36);
		transition: background-color var(--transition-fast);
	}
	.collapsed {
		display: flex;
		flex-direction: row-reverse;
		align-items: center;
		justify-content: space-between;
		padding: var(--space-10) var(--space-8);
		border-radius: var(--radius-m);
		gap: var(--space-8);
		transition: background-color var(--transition-fast);
		height: initial;
	}
	.collapsed-txt {
		transform: rotate(180deg);
		flex-shrink: 0;
	}

	.icon {
		flex-shrink: 0;
	}

	.domain-button:not(.selected):hover,
	.domain-button:not(.selected):focus,
	.selected {
		background-color: color-mix(in srgb, transparent, var(--darken-tint-light));
	}
</style>
