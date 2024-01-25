<script lang="ts">
	import IconButton from '$lib/components/IconButton.svelte';
	import { getVSIFileIcon } from '$lib/ext-icons';
	import type { File } from '$lib/vbranches/types';
	import { createEventDispatcher } from 'svelte';
	import Tag from './Tag.svelte';
	import { computeFileStatus, computedAddedRemoved } from '$lib/vbranches/fileStatus';
	import FileStatusTag from './FileStatusTag.svelte';

	export let file: File;
	export let isFileLocked: boolean;

	const dispatch = createEventDispatcher<{ close: void }>();
	$: fileStats = computedAddedRemoved(file);
	$: fileStatus = computeFileStatus(file);

	function boldenFilename(filepath: string): { filename: string; path: string } {
		const parts = filepath.split('/');
		if (parts.length === 0) return { filename: '', path: '' };

		const filename = parts[parts.length - 1];
		const path = parts.slice(0, -1).join('/');

		return { filename, path };
	}

	$: fileTitle = boldenFilename(file.path);
</script>

<div class="header">
	<div class="header__inner">
		<img src={getVSIFileIcon(file.path)} alt="js" width="13" height="13" class="icon" />
		<div class="header__info truncate">
			<div class="header__filetitle text-base-13 truncate">
				<span class="header__filename">{fileTitle.filename}</span>
				<span class="header__filepath">{fileTitle.path}</span>
			</div>
			<div class="header__tags">
				{#if file.conflicted || isFileLocked}
					<div class="header__tag-group">
						{#if isFileLocked}
							<Tag
								icon="locked-small"
								color="warning"
								help="File changes cannot be moved because part of this file was already committed into this branch"
								border>Locked</Tag
							>
						{/if}
						{#if file.conflicted}
							<Tag icon="warning-small" color="error" border>Has conflicts</Tag>
						{/if}
					</div>
				{/if}
				<div class="header__tag-group">
					{#if fileStats.added}
						<Tag color="success" border>+{fileStats.added}</Tag>
					{/if}
					{#if fileStats.removed}
						<Tag color="error" border>-{fileStats.removed}</Tag>
					{/if}
					{#if fileStatus}
						<FileStatusTag status={fileStatus} />
					{/if}
				</div>
			</div>
		</div>
	</div>
	<IconButton icon="cross" size="m" on:click={() => dispatch('close')} />
</div>

<style lang="postcss">
	.header {
		display: flex;
		padding: var(--space-16);
		gap: var(--space-12);
		border-bottom: 1px solid var(--clr-theme-container-outline-light);
	}
	.header__inner {
		display: flex;
		flex-grow: 1;
		gap: var(--space-6);
		overflow: hidden;
	}
	.header__info {
		display: flex;
		flex-direction: column;
		gap: var(--space-8);
		width: 100%;
	}
	.header__tags {
		display: flex;
		gap: var(--space-6);
	}
	.header__tag-group {
		display: flex;
		gap: var(--space-2);
	}
	.header__filetitle {
		width: 100%;
	}
	.header__filename {
		color: var(--clr-theme-scale-ntrl-0);
		line-height: 120%;
	}
	.header__filepath {
		color: var(--clr-theme-scale-ntrl-50);
	}
	.icon {
		flex-shrink: 0;
		width: var(--space-14);
		height: var(--space-14);
		margin-top: calc(var(--space-2) / 2);
	}
</style>
