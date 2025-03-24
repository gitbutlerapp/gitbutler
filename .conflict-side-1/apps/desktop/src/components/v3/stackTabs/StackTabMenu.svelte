<script lang="ts">
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';

	type Props = {
		projectId: string;
		stackId: string;
		isOpen?: boolean;
	};

	let { projectId, stackId, isOpen = $bindable() }: Props = $props();

	const stackService = getContext(StackService);
	const [unapply] = stackService.unapply;

	let trigger = $state<HTMLElement>();
	let contextMenu = $state<ContextMenu>();
</script>

<button
	aria-label="Stack menu"
	class="menu-button focus-state"
	class:menu-open={isOpen}
	onclick={(e) => {
		e.preventDefault();
		e.stopPropagation();
		contextMenu?.toggle();
	}}
	bind:this={trigger}
	type="button"
>
	<div class="menu-button-dots"></div>
</button>

<ContextMenu
	bind:this={contextMenu}
	leftClickTrigger={trigger}
	ontoggle={(flag) => {
		isOpen = flag;
	}}
	side="bottom"
>
	<ContextMenuSection>
		<ContextMenuItem
			label="Unapply Stack"
			keyboardShortcut="$mod+X"
			onclick={async () => {
				await unapply({ projectId, stackId });
				contextMenu?.close();
			}}
		/>
		<ContextMenuItem
			label="Rename"
			keyboardShortcut="$mod+R"
			onclick={() => {
				contextMenu?.close();
			}}
		/>
	</ContextMenuSection>
</ContextMenu>

<style lang="postcss">
	.menu-button {
		position: relative;
		display: flex;
		width: var(--menu-btn-size);
		height: var(--menu-btn-size);
		align-items: center;
		justify-content: center;
		color: var(--clr-text-2);
		border-radius: var(--radius-s);

		&.menu-open,
		&:hover,
		&:focus-within {
			color: var(--clr-text-1);
		}
	}

	.menu-button-dots {
		position: absolute;
		top: 50%;
		left: 50%;
		transform: translate(-50%, -50%) scale(0.9);
		width: 3px;
		height: 3px;
		border-radius: 50%;
		background-color: currentColor;

		&::after,
		&::before {
			content: '';
			position: absolute;
			top: 0;
			width: 3px;
			height: 3px;
			border-radius: 50%;
			background-color: currentColor;
		}

		&::after {
			left: 6px;
		}
		&::before {
			left: -6px;
		}
	}
</style>
