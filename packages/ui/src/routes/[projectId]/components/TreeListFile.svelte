<script lang="ts">
	import { draggableFile } from '$lib/draggables';
	import { getVSIFileIcon } from '$lib/ext-icons';
	import Icon from '$lib/icons/Icon.svelte';
	import { draggable } from '$lib/utils/draggable';
	import type { File } from '$lib/vbranches/types';
	import FileStatusIcons from './FileStatusIcons.svelte';

	export let branchId: string;
	export let file: File;
	export let selected: boolean;
	export let readonly: boolean;
</script>

<div
	use:draggable={{
		...draggableFile(branchId, file),
		disabled: readonly
	}}
	on:click
	on:keydown
	class="draggable-wrapper"
	role="button"
	tabindex="0"
>
	<div class="tree-list-file" class:selected>
		<div class="dot">
			<Icon name="dot" />
		</div>
		<div class="icon">
			<img
				src={getVSIFileIcon(file.path)}
				alt="js"
				width="12"
				style="width: 0.75rem"
				class="mr-1 inline"
			/>
		</div>
		<span class="name text-base-12">
			{file.filename}
		</span>
		<FileStatusIcons {file} />
	</div>
</div>

<style lang="postcss">
	.draggable-wrapper {
		display: inline-block;
	}
	.tree-list-file {
		display: inline-flex;
		align-items: center;
		padding: var(--space-4) var(--space-8) var(--space-4) var(--space-4);
		gap: var(--space-6);
		border-radius: var(--radius-s);
		max-width: 100%;
		background: var(--clr-theme-container-light);
		&:not(.selected):hover {
			background: var(--clr-theme-container-pale);
		}
		overflow: hidden;
	}
	.name {
		color: var(--clr-theme-scale-ntrl-0);
		text-overflow: ellipsis;
		overflow: hidden;
	}
	.dot {
		color: var(--clr-theme-scale-ntrl-0);
		opacity: 0.3;
	}
	.selected {
		background-color: var(--clr-theme-scale-pop-80);
	}
</style>
