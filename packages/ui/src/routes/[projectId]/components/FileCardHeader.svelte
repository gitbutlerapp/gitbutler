<script lang="ts">
	import IconButton from '$lib/components/IconButton.svelte';
	import Tooltip from '$lib/components/Tooltip.svelte';
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

	function boldenFilename(filepath: string): string {
		const parts = filepath.split('/');
		if (parts.length == 0) return '';
		return (
			'<span class="font-semibold text-color-2 mr-1">' +
			parts[parts.length - 1] +
			'</span>/' +
			parts.slice(0, -1).join('/')
		);
	}
</script>

<div class="header">
	<div class="header__inner">
		<img
			src={getVSIFileIcon(file.path)}
			alt="js"
			width="13"
			height="13"
			style="width: 0.8125rem; height: 0.8125rem"
			class="inline"
		/>
		<div class="header__info">
			<div class="header__filename text-base-13">
				{@html boldenFilename(file.path)}
			</div>
			<div class="header__tags">
				<div class="header__tag-group">
					{#if isFileLocked}
						<Tooltip
							label="File changes cannot be moved because part of this file was already committed into this branch"
						>
							<Tag icon="locked-small" color="warning" border>Locked</Tag>
						</Tooltip>
					{/if}
					{#if file.conflicted}
						<Tag icon="warning-small" color="error" border>Has conflicts</Tag>
					{/if}
				</div>
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
	<div class="header__close">
		<IconButton icon="cross" on:click={() => dispatch('close')} />
	</div>
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
	}
	.header__info {
		display: flex;
		flex-direction: column;
		gap: var(--space-6);
	}
	.header__tags {
		display: flex;
		gap: var(--space-6);
	}
	.header__tag-group {
		display: flex;
		gap: var(--space-2);
	}
	.header__filename {
		color: var(--clr-theme-scale-ntrl-50);
	}
	.header__close {
	}
</style>
