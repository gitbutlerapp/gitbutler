<script lang="ts">
	import FileStatusCircle from './FileStatusCircle.svelte';
	import Icon from '$lib/components/Icon.svelte';
	import { computeFileStatus } from '$lib/vbranches/fileStatus';
	import type { File } from '$lib/vbranches/types';

	export let file: File;
	$: isLocked = file.hunks.some((h) => h.locked);
</script>

<div class="file-status">
	<div class="file-status__icons">
		{#if isLocked}
			<div class="locked">
				<Icon name="locked-small" color="warn" />
			</div>
		{/if}
		{#if file.conflicted}
			<div class="conflicted">
				<Icon name="warning-small" color="error" />
			</div>
		{/if}
	</div>
	<div class="status">
		<FileStatusCircle status={computeFileStatus(file)} />
	</div>
</div>

<style lang="postcss">
	.file-status {
		display: flex;
		align-items: center;
		gap: var(--space-4);
	}
	.file-status__icons {
		display: flex;
	}
</style>
