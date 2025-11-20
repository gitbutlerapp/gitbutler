<script lang="ts">
	import CollapseStackButton from '$components/CollapseStackButton.svelte';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/core/context';
	import { ContextMenuItem, ContextMenuSection, Icon, KebabButton } from '@gitbutler/ui';
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

	<KebabButton minimal>
		{#snippet contextMenu({ close })}
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
	</KebabButton>
</div>

<style lang="postcss">
	.drag-handle-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		height: 28px;
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
		padding: 3px;
		border-radius: var(--radius-s);
		background-color: var(--clr-bg-2);
		color: var(--clr-text-3);
		pointer-events: none;
	}
</style>
