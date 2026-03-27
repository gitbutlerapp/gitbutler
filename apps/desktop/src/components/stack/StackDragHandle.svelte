<script lang="ts">
	import CollapseStackButton from "$components/branch/CollapseStackButton.svelte";
	import { IRC_SESSION_BRIDGE } from "$lib/irc/sessionBridge.svelte";
	import { SETTINGS_SERVICE } from "$lib/settings/appSettings";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { inject } from "@gitbutler/core/context";
	import { ContextMenuItem, ContextMenuSection, Icon, KebabButton } from "@gitbutler/ui";
	import type { Stack } from "$lib/stacks/stack";

	type Props = {
		stackId?: string;
		projectId: string;
		branchName?: string;
		disabled?: boolean;
		onFold?: () => void;
		onOptionsClick?: () => void;
	};

	let { stackId, projectId, branchName, disabled = false, onFold }: Props = $props();

	const stackService = inject(STACK_SERVICE);
	const settingsService = inject(SETTINGS_SERVICE);
	const ircSessionBridge = inject(IRC_SESSION_BRIDGE);

	const settingsStore = settingsService.appSettings;
	const ircEnabled = $derived(
		($settingsStore?.featureFlags.irc && $settingsStore?.irc?.connection?.enabled) ?? false,
	);
	const isManuallyBridged = $derived(ircSessionBridge.isManuallyBridged(stackId));

	function toggleBridging() {
		if (!stackId) return;
		ircSessionBridge.setManualBridge(stackId, !isManuallyBridged.current);
	}

	// Get all stacks to determine if we can move left/right
	const stacksQuery = $derived(stackService.stacks(projectId));
	const stacks = $derived(stacksQuery.response ?? []);

	const currentStackIndex = $derived(
		stackId ? stacks.findIndex((stack: Stack) => stack.id === stackId) : -1,
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
				.filter((s): s is { id: string; order: number } => s !== undefined),
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
				.filter((s): s is { id: string; order: number } => s !== undefined),
		});

		// Refetch to update the UI
		await stacksQuery.result.refetch();
	}

	async function unapplyStack() {
		if (!stackId) return;

		await stackService.unapply({
			projectId,
			stackId,
		});

		// Refetch to update the UI
		await stacksQuery.result.refetch();
	}
</script>

<div class="drag-handle-row" data-remove-from-panning data-drag-handle draggable="true">
	<CollapseStackButton {disabled} onClick={onFold} />

	<div class="drag-handle-icon">
		<Icon name="drag-horizontal" />
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
			{#if ircEnabled && stackId && branchName}
				<ContextMenuSection>
					<ContextMenuItem
						label="IRC Bot"
						icon={isManuallyBridged.current ? "tick" : undefined}
						onclick={() => {
							toggleBridging();
							close();
						}}
					/>
				</ContextMenuSection>
			{/if}
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
		color: var(--text-3);
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
		background-color: var(--bg-2);
		color: var(--text-3);
		pointer-events: none;
	}
</style>
