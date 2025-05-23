<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';
	import DomainButton from '$components/DomainButton.svelte';

	interface Props {
		href: string;
		isNavCollapsed: boolean;
	}

	const { href, isNavCollapsed }: Props = $props();
	const label = 'Feeds';
</script>

<DomainButton
	isSelected={$page.url.pathname === href}
	{isNavCollapsed}
	tooltipLabel={label}
	onmousedown={async () => await goto(href)}
>
	<img class="icon" src="/images/domain-icons/working-branches.svg" alt="" />
	{#if !isNavCollapsed}
		<span class="text-14 text-semibold" class:collapsed-txt={isNavCollapsed}>{label}</span>
	{/if}
</DomainButton>

<style lang="postcss">
	.icon {
		flex-shrink: 0;
		width: 20px;
		height: 20px;
		border-radius: var(--radius-s);
	}
</style>
