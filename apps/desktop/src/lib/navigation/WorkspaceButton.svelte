<script lang="ts">
	import DomainButton from './DomainButton.svelte';
	import UpdateBaseButton from '../components/UpdateBaseButton.svelte';
	import { BaseBranch } from '$lib/baseBranch/baseBranch';
	import { getContextStore } from '$lib/utils/context';
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

<DomainButton
	isSelected={selected}
	{isNavCollapsed}
	tooltipLabel={label}
	onmousedown={async () => await goto(href)}
>
	{#if domain === 'workspace'}
		<img class="icon" src={iconSrc} alt="" />

		{#if !isNavCollapsed}
			<span class="text-14 text-semibold" class:collapsed-txt={isNavCollapsed}>{label}</span>
			{#if ($baseBranch?.behind || 0) > 0 && !isNavCollapsed}
				<UpdateBaseButton />
			{/if}
		{/if}
	{:else}
		<slot />
	{/if}
</DomainButton>

<style lang="postcss">
	.icon {
		border-radius: var(--radius-s);
		height: 20px;
		width: 20px;
		flex-shrink: 0;
	}
</style>
