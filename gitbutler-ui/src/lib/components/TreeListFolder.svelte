<script lang="ts">
	import Checkbox from '$lib/components/Checkbox.svelte';
	import Icon from '$lib/components/Icon.svelte';
	import { maybeGetContextStore } from '$lib/utils/context';
	import { Ownership } from '$lib/vbranches/ownership';
	import type { TreeNode } from '$lib/vbranches/filetree';
	import type { Hunk, RemoteHunk } from '$lib/vbranches/types';
	import type { Writable } from 'svelte/store';

	export let expanded: boolean;
	export let node: TreeNode;
	export let isChecked = false;
	export let showCheckbox = false;
	export let isIndeterminate = false;

	const selectedOwnership: Writable<Ownership> | undefined = maybeGetContextStore(Ownership);

	function idWithChildren(node: TreeNode): [string, (Hunk | RemoteHunk)[]][] {
		if (node.file) {
			return [[node.file.id, node.file.hunks]];
		}
		return node.children.flatMap(idWithChildren);
	}

	function onSelectionChanged() {
		idWithChildren(node).forEach(([fileId, hunks]) => {
			if (isChecked) {
				selectedOwnership?.update((ownership) =>
					ownership.remove(fileId, ...hunks.map((h) => h.id))
				);
			} else {
				selectedOwnership?.update((ownership) => ownership.add(fileId, ...hunks));
			}
		});
	}
</script>

<button class="tree-list-folder" class:expanded on:click on:mousedown>
	<div class="chevron-icon" class:chevron-expanded={expanded}>
		<Icon name="chevron-down-small" />
	</div>
	<div class="content-wrapper">
		{#if showCheckbox}
			<Checkbox
				small
				checked={isChecked}
				indeterminate={isIndeterminate}
				on:change={onSelectionChanged}
			/>
		{/if}
		<div class="name-wrapper">
			<!-- folder-icon.svg -->
			<svg
				style="width: var(--size-12)"
				viewBox="0 0 12 12"
				fill="none"
				xmlns="http://www.w3.org/2000/svg"
			>
				<path
					d="M0 1C0 0.447715 0.447715 0 1 0H5C5.36931 0 5.70856 0.203548 5.88235 0.529412L6.91765 2.47059C7.09144 2.79645 7.43069 3 7.8 3H11C11.5523 3 12 3.44772 12 4V11C12 11.5523 11.5523 12 11 12H1C0.447715 12 0 11.5523 0 11V1Z"
					fill="url(#paint0_linear_1539_3024)"
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

			<span class="name text-base-12">
				{node.name}
			</span>
		</div>
	</div>
</button>

<style lang="postcss">
	.tree-list-folder {
		display: flex;
		align-items: center;
		height: var(--size-28);
		width: 100%;
		padding: var(--size-4) var(--size-8) var(--size-4) var(--size-4);
		gap: var(--size-6);
		border-radius: var(--radius-s);
		margin-bottom: var(--size-2);
		&:hover {
			background: color-mix(in srgb, var(--clr-theme-container-light), var(--darken-tint-light));

			& .chevron-icon {
				opacity: 0.7;
			}
		}
	}
	.content-wrapper {
		display: flex;
		align-items: center;
		gap: var(--size-10);
	}
	.name-wrapper {
		display: flex;
		align-items: center;
		gap: var(--size-6);
	}
	.name {
		color: var(--clr-theme-scale-ntrl-0);
	}
	.chevron-icon {
		opacity: 0.5;
		transform: rotate(-90deg);
		transition:
			opacity var(--transition-fast),
			transform var(--transition-fast);
	}
	.chevron-expanded {
		transform: rotate(0deg);
	}
</style>
