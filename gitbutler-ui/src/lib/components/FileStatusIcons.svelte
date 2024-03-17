<script lang="ts">
	import FileStatusCircle from './FileStatusCircle.svelte';
	import Icon from '$lib/components/Icon.svelte';
	import { computeFileStatus } from '$lib/utils/fileStatus';
	import type { AnyFile } from '$lib/vbranches/types';

	export let file: AnyFile;
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
		gap: var(--size-4);
	}
	.file-status__icons {
		display: flex;
	}
</style>
