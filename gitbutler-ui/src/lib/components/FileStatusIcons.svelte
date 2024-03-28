<script lang="ts">
	import FileStatusCircle from './FileStatusCircle.svelte';
	import Icon from '$lib/components/Icon.svelte';
	import { getContextStoreBySymbol } from '$lib/utils/context';
	import { computeFileStatus } from '$lib/utils/fileStatus';
	import { tooltip } from '$lib/utils/tooltip';
	import { getLockText } from '$lib/vbranches/tooltip';
	import { type AnyFile, Commit, LOCAL_COMMITS, LocalFile } from '$lib/vbranches/types';

	export let file: AnyFile;

	const localCommits =
		file instanceof LocalFile ? getContextStoreBySymbol<Commit[]>(LOCAL_COMMITS) : undefined;

	$: lockedIds = file.lockedIds;
	$: lockText = lockedIds.length > 0 && $localCommits ? getLockText(lockedIds, $localCommits) : '';
</script>

<div class="file-status">
	<div class="file-status__icons">
		{#if lockText}
			<div class="locked" use:tooltip={{ text: lockText, delay: 500 }}>
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
