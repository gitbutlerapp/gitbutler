<script lang="ts">
	import { countLeafNodes, getAllChanges, nodePath, type TreeNode } from '$lib/files/filetreeV3';
	import { ChangeSelectionService } from '$lib/selection/changeSelection.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Checkbox from '@gitbutler/ui/Checkbox.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';

	type Props = {
		node: TreeNode & { kind: 'dir' };
		showCheckbox: boolean;
	};

	const { node, showCheckbox }: Props = $props();
	const expanded = true;

	const selectionService = getContext(ChangeSelectionService);
	const selection = $derived(selectionService.getByPrefix(nodePath(node)));
	const selectionCount = $derived(selection.current.length);
	const fileCount = $derived(countLeafNodes(node));

	const indeterminate = $derived.by(() => {
		if (!showCheckbox) return false;
		return selectionCount !== 0 && selectionCount !== fileCount;
	});

	const checked = $derived.by(() => {
		if (!showCheckbox) return false;
		return selectionCount === fileCount;
	});
</script>

<div class="tree-list-folder" class:expanded>
	<div class="chevron-icon" class:chevron-expanded={expanded}>
		<Icon size={15} name="chevron-down-small" />
	</div>
	<div class="content-wrapper">
		{#if showCheckbox}
			<Checkbox
				small
				{checked}
				{indeterminate}
				onchange={(e) => {
					const changes = getAllChanges(node);
					for (const change of changes) {
						if (e.currentTarget.checked) {
							selectionService.add({
								type: 'full',
								path: change.path,
								pathBytes: change.pathBytes
							});
						} else {
							selectionService.remove(change.path);
						}
					}
				}}
			/>
			<!-- onchange={onSelectionChanged} -->
		{/if}
		<div class="name-wrapper">
			<!-- folder-icon.svg -->
			<svg
				style="width: 16px; height: 16px; flex-shrink: 0;"
				viewBox="0 0 12 12"
				fill="none"
				xmlns="http://www.w3.org/2000/svg"
			>
				<path
					d="M0 1C0 0.447715 0.447715 0 1 0H5C5.36931 0 5.70856 0.203548 5.88235 0.529412L6.91765 2.47059C7.09144 2.79645 7.43069 3 7.8 3H11C11.5523 3 12 3.44772 12 4V11C12 11.5523 11.5523 12 11 12H1C0.447715 12 0 11.5523 0 11V1Z"
					fill="#44BEF2"
				/>
				<defs>
					<linearGradient
						id="paint0_linear_1539_3024"
						x1="6"
						y1="0"
						x2="6"
						y2="12"
						gradientUnits="userSpaceOnUse"
					>
						<stop offset="0.145833" stop-color="#60A5FA" />
						<stop offset="1" stop-color="#177FFF" />
					</linearGradient>
				</defs>
			</svg>

			<span class="dirname">
				{node.name}
			</span>
		</div>
	</div>
</div>

<style lang="postcss">
	.tree-list-folder {
		display: flex;
		align-items: center;
		width: 100%;
		padding: 0 8px 0 0;
		gap: 6px;
		border-radius: var(--radius-s);
		margin: 6px 0;
		&:hover {
			background: color-mix(in srgb, var(--clr-container-light), var(--darken-tint-light));

			& .chevron-icon {
				opacity: 0.7;
			}
		}
	}
	.content-wrapper {
		display: flex;
		align-items: center;
		gap: 10px;
		overflow: hidden;
	}
	.name-wrapper {
		display: flex;
		align-items: baseline;
		gap: 6px;
		width: 100%;
	}
	.dirname {
		color: var(--clr-text-1);
		text-overflow: ellipsis;
		white-space: nowrap;
		overflow: hidden;
	}
	.chevron-icon {
		display: flex;
		opacity: 0.4;
		width: 15px;
		height: 15px;
		transform: rotate(-90deg);
		transition:
			opacity var(--transition-fast),
			transform var(--transition-fast);
	}
	.chevron-expanded {
		transform: rotate(0deg);
	}
</style>
