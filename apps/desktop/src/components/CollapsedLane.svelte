<script lang="ts">
	import CollapseStackButton from "$components/CollapseStackButton.svelte";
	import { Icon } from "@gitbutler/ui";

	type Props = {
		stackId?: string;
		branchNames?: string[];
		projectId: string;
	};

	const { stackId, branchNames, projectId }: Props = $props();

	let toggleFold: (() => void) | undefined = $state();

	function handleDoubleClick() {
		if (toggleFold) {
			toggleFold();
		}
	}
</script>

<div
	role="presentation"
	class="folded-lane"
	data-remove-from-panning
	data-drag-handle
	draggable="true"
	ondblclick={handleDoubleClick}
>
	<CollapseStackButton {stackId} {projectId} isFolded onToggle={(fn) => (toggleFold = fn)} />

	<div class="drag-handle-icon">
		<Icon name="draggable-wide" />
	</div>

	<div class="text-14 text-semibold stack-names">
		{#if branchNames && branchNames.length > 0}
			{#each branchNames as branchName}
				<span class="branch-name">{branchName}</span>

				{#if branchName !== branchNames[branchNames.length - 1]}
					<Icon name="text-link" color="var(--clr-text-3)" rotate={90} />
				{/if}
			{/each}
		{:else}
			<span class="branch-name">Folded Stack</span>
		{/if}
	</div>
</div>

<style lang="postcss">
	.folded-lane {
		display: flex;
		flex-direction: column;
		align-items: center;
		height: 100%;
		padding: 2px 9px 18px;
		border-right: 1px solid var(--clr-border-2);
		background: var(--clr-bg-2);
		--lighter-bg-drop-shadow: color-mix(in srgb, var(--clr-drop-shadow) 50%, var(--clr-bg-2));
		box-shadow: inset -5px 0 10px var(--lighter-bg-drop-shadow);
		color: var(--clr-text-3);
		cursor: grab;
	}

	.drag-handle-icon {
		display: flex;
		margin-top: 4px;
		pointer-events: none;
	}

	.stack-names {
		display: flex;
		align-items: center;
		margin-top: 10px;
		overflow: hidden;
		gap: 10px;
		text-orientation: mixed;
		pointer-events: none;
		writing-mode: vertical-lr;
	}

	.branch-name {
		overflow: hidden;
		color: var(--clr-text-1);
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>
