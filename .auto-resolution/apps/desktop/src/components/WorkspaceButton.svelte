<script lang="ts">
	import DomainButton from './DomainButton.svelte';
	import UpdateBaseButton from '$components/UpdateBaseButton.svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';

	interface Props {
		href: string;
		isNavCollapsed: boolean;
	}

	const { href, isNavCollapsed }: Props = $props();

	const label = 'Workspace';
</script>

<DomainButton
	isSelected={$page.url.pathname === href}
	{isNavCollapsed}
	tooltipLabel={label}
	onmousedown={async () => await goto(href)}
>
	<img class="icon" src={'/images/domain-icons/working-branches.svg'} alt="" />

	{#if !isNavCollapsed}
		<span class="text-14 text-semibold" class:collapsed-txt={isNavCollapsed}>{label}</span>
		{#if !isNavCollapsed}
			<UpdateBaseButton />
		{/if}
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
