<script lang="ts">
	import { draggableFile } from '$lib/draggables';
	import { getVSIFileIcon } from '$lib/ext-icons';
	import { draggable } from '$lib/utils/draggable';
	import type { File } from '$lib/vbranches/types';
	import FileStatusIcons from './FileStatusIcons.svelte';

	export let branchId: string;
	export let file: File;
	export let readonly: boolean;
	export let selected: boolean;
</script>

<div
	on:click
	on:keydown
	use:draggable={{
		...draggableFile(branchId, file),
		disabled: readonly
	}}
	role="button"
	tabindex="0"
>
	<div class="file-list-item" id={`file-${file.id}`} class:selected>
		<div class="info">
			<div class="icon">
				<img
					src={getVSIFileIcon(file.path)}
					alt="js"
					width="12"
					style="width: 0.75rem"
					class="mr-1 inline"
				/>
			</div>
			<span class="text-base-12 name">
				{file.filename}
			</span>
			<span class="text-base-12 path">
				{file.justpath}
			</span>
		</div>
		<FileStatusIcons {file} />
	</div>
</div>

<style lang="postcss">
	.file-list-item {
		display: flex;
		align-items: center;
		padding: var(--space-4) var(--space-8);
		gap: var(--space-16);
		border-radius: var(--radius-s);
		max-width: 100%;
		overflow: hidden;
		background: var(--clr-theme-container-light);
		text-align: left;
		&:not(.selected):hover {
			background: var(--clr-theme-container-pale);
		}
	}
	.icon {
		flex-shrink: 0;
	}
	.info {
		display: flex;
		align-items: center;
		flex-grow: 1;
		flex-shrink: 1;
		gap: var(--space-6);
		overflow: hidden;
	}
	.name {
		color: var(--clr-theme-scale-ntrl-0);
		white-space: nowrap;
		flex-shrink: 0;
		text-overflow: ellipsis;
		overflow: hidden;
	}
	.path {
		color: var(--clr-theme-scale-ntrl-40);
		flex-shrink: 1;
		white-space: nowrap;
		text-overflow: ellipsis;
		overflow: hidden;
	}
	.locked {
		color: var(--clr-theme-scale-warn-60);
	}
	.selected {
		background-color: var(--clr-theme-scale-pop-80);
	}
</style>
