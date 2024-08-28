<script lang="ts">
	import { Project } from '$lib/backend/projects';
	import DomainButton from '$lib/navigation/DomainButton.svelte';
	import { getContext } from '$lib/utils/context';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';

	interface Props {
		isNavCollapsed: boolean;
	}

	const { isNavCollapsed }: Props = $props();

	const project = getContext(Project);
</script>

<DomainButton
	isSelected={$page.url.pathname === `/${project.id}/edit`}
	{isNavCollapsed}
	tooltipLabel="Edit"
	onmousedown={async () => await goto(`/${project.id}/edit`)}
>
	<img class="icon" src="/images/domain-icons/working-branches.svg" alt="" />
	<span class="text-14 text-semibold" class:collapsed-txt={isNavCollapsed}>Edit</span>
</DomainButton>

<style lang="postcss">
	.icon {
		border-radius: var(--radius-s);
		height: 20px;
		width: 20px;
		flex-shrink: 0;
	}
</style>
