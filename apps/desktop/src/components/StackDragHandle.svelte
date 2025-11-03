<script lang="ts">
	import CollapseStackButton from '$components/CollapseStackButton.svelte';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/core/context';
	import { ContextMenu, ContextMenuItem, ContextMenuSection, Icon } from '@gitbutler/ui';
	import type { Stack } from '$lib/stacks/stack';

	type Props = {
		stackId?: string;
		projectId: string;
		disabled?: boolean;
		onOptionsClick?: () => void;
	};

	let { stackId, projectId, disabled = false }: Props = $props();

	const stackService = inject(STACK_SERVICE);

	// Get all stacks to determine if we can move left/right
	const stacksQuery = stackService.stacks(projectId);
	const stacks = $derived(stacksQuery.response ?? []);

	let contextMenu = $state<ReturnType<typeof ContextMenu>>();
	let kebabButton = $state<HTMLButtonElement>();

	const currentStackIndex = $derived(
		stackId ? stacks.findIndex((stack: Stack) => stack.id === stackId) : -1
	);
	const canMoveLeft = $derived(currentStackIndex > 0);
	const canMoveRight = $derived(currentStackIndex >= 0 && currentStackIndex < stacks.length - 1);

	async function moveStackLeft() {
		if (!stackId || !canMoveLeft) return;

		const newStacks = [...stacks];
		const [removed] = newStacks.splice(currentStackIndex, 1);
		if (!removed) return;

		// Insert at the beginning (leftmost position)
		newStacks.unshift(removed);

		await stackService.updateStackOrder({
			projectId,
			stacks: newStacks
				.map((stack, i) => (stack.id ? { id: stack.id, order: i } : undefined))
				.filter((s): s is { id: string; order: number } => s !== undefined)
		});

		// Refetch to update the UI
		await stacksQuery.result.refetch();
	}

	async function moveStackRight() {
		if (!stackId || !canMoveRight) return;

		const newStacks = [...stacks];
		const [removed] = newStacks.splice(currentStackIndex, 1);
		if (!removed) return;

		// Insert at the end (rightmost position)
		newStacks.push(removed);

		await stackService.updateStackOrder({
			projectId,
			stacks: newStacks
				.map((stack, i) => (stack.id ? { id: stack.id, order: i } : undefined))
				.filter((s): s is { id: string; order: number } => s !== undefined)
		});

		// Refetch to update the UI
		await stacksQuery.result.refetch();
	}

	async function unapplyStack() {
		if (!stackId) return;

		await stackService.unapply({
			projectId,
			stackId
		});

		// Refetch to update the UI
		await stacksQuery.result.refetch();
	}
</script>

<div class="drag-handle-row" data-remove-from-panning data-drag-handle draggable="true">
	<CollapseStackButton {stackId} {projectId} {disabled} />

	<div class="drag-handle-icon">
		<Icon name="draggable-wide" />
	</div>

	<button
		type="button"
		class="kebab-btn"
		{disabled}
		bind:this={kebabButton}
		onclick={(e) => {
			e.stopPropagation();
			contextMenu?.toggle();
		}}
	>
		<Icon name="kebab" zIndex="var(--z-ground)" />
	</button>

	<ContextMenu bind:this={contextMenu} leftClickTrigger={kebabButton}>
		{#snippet menu({ close })}
			<ContextMenuSection>
				<ContextMenuItem
					label="Move to leftmost"
					icon="leftmost-lane"
					disabled={!canMoveLeft}
					onclick={() => {
						moveStackLeft();
						close();
					}}
				/>
				<ContextMenuItem
					label="Move to rightmost"
					icon="rightmost-lane"
					disabled={!canMoveRight}
					onclick={() => {
						moveStackRight();
						close();
					}}
				/>
			</ContextMenuSection>
			<ContextMenuSection>
				<ContextMenuItem
					label="Unapply stack"
					icon="eject"
					onclick={() => {
						unapplyStack();
						close();
					}}
				/>
			</ContextMenuSection>
		{/snippet}
	</ContextMenu>
</div>

<style lang="postcss">
	.drag-handle-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		height: 16px;
		height: 24px;
		margin: 0 -3px;
		color: var(--clr-text-3);
		cursor: grab;
		transition:
			height var(--transition-medium),
			color var(--transition-fast);
	}

	.drag-handle-icon {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		justify-content: center;
		width: 18px;
		height: 10px;
		padding: 2px;
		border-radius: 3px;
		background-color: var(--clr-bg-2);
		color: var(--clr-text-3);
		pointer-events: none;
	}

	.kebab-btn {
		display: flex;
		position: relative;
		align-items: center;
		justify-content: center;
		width: 24px;
		height: 24px;
		color: var(--clr-text-1);
		opacity: 0.5;
		transition: opacity var(--transition-fast);

		&:hover {
			opacity: 0.9;
		}

		&:after {
			z-index: 0;
			position: absolute;
			top: 50%;
			left: 50%;
			width: 20px;
			height: 8px;
			transform: translate(-50%, -50%);
			border-radius: var(--radius-m);
			background-color: var(--clr-bg-2);
			/* background-color: rgb(255, 165, 165); */
			content: '';
		}
	}
</style>
