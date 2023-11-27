<script lang="ts">
	import { getVSIFileIcon } from '$lib/ext-icons';
	import Icon from '$lib/icons/Icon.svelte';
	import { computeFileStatus } from '$lib/vbranches/fileStatus';
	import type { File } from '$lib/vbranches/types';
	import FileStatus from './FileStatus.svelte';

	export let file: File;
</script>

<button on:click class="tree-list-file">
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
	<div class="name flex-shrink">
		{file.filename}
	</div>
	<div class="status">
		<FileStatus status={computeFileStatus(file)} />
	</div>
</button>

<style lang="postcss">
	.tree-list-file {
		display: flex;
		align-items: center;
		padding: var(--space-4);
		gap: var(--space-6);
		border-radius: var(--radius-s);
		max-width: 100%;
		&:hover {
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
</style>
