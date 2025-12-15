<script lang="ts">
	import Badge from '$components/Badge.svelte';
	import Icon from '$components/Icon.svelte';
	import Tooltip from '$components/Tooltip.svelte';
	import type { FileStatus } from '$components/file/types';
	import type { ComponentColorType } from '$lib/utils/colorTypes';

	interface Props {
		status: FileStatus;
		style?: 'dot' | 'full' | 'full-large';
		tooltip?: string;
	}

	const { status, style = 'full', tooltip }: Props = $props();

	const TOOLTIP_MAX_WIDTH = 320;

	function getFullStatusText(status: FileStatus): string {
		switch (status) {
			case 'addition':
				return 'Added';
			case 'modification':
				return 'Modified';
			case 'deletion':
				return 'Deleted';
			case 'rename':
				return 'Renamed';
			default:
				return status;
		}
	}

	function getStatusColor(status: FileStatus): ComponentColorType {
		switch (status) {
			case 'addition':
				return 'success';
			case 'modification':
				return 'warning';
			case 'deletion':
				return 'error';
			case 'rename':
				return 'purple';
			default:
				return 'gray';
		}
	}
</script>

{#if style === 'dot'}
	<Tooltip text={!tooltip ? getFullStatusText(status) : tooltip} maxWidth={TOOLTIP_MAX_WIDTH}>
		<div
			class="status-dot-wrap"
			class:added={status === 'addition'}
			class:modified={status === 'modification'}
			class:deleted={status === 'deletion'}
			class:renamed={status === 'rename'}
		>
			{#if status === 'addition'}
				<Icon name="file-added" />
			{:else if status === 'modification'}
				<Icon name="file-modified" />
			{:else if status === 'deletion'}
				<Icon name="file-deleted" />
			{:else if status === 'rename'}
				<Icon name="file-moved" />
			{/if}
		</div>
	</Tooltip>
{:else if style === 'full'}
	<Tooltip text={status === 'rename' && tooltip ? tooltip : undefined} maxWidth={TOOLTIP_MAX_WIDTH}>
		<Badge style={getStatusColor(status)} kind="soft">{getFullStatusText(status)}</Badge>
	</Tooltip>
{:else if style === 'full-large'}
	<Tooltip text={status === 'rename' && tooltip ? tooltip : undefined} maxWidth={TOOLTIP_MAX_WIDTH}>
		<Badge style={getStatusColor(status)} kind="soft" size="tag">{getFullStatusText(status)}</Badge>
	</Tooltip>
{/if}

<style lang="postcss">
	.status-dot-wrap {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		justify-content: center;
		width: fit-content;
		width: 16px;
		height: 16px;
		color: var(--file-dot-color);
	}

	/* MODIFIERS */
	.status-dot-wrap.added {
		--file-dot-color: var(--clr-change-icon-addition);
	}
	.status-dot-wrap.modified {
		--file-dot-color: var(--clr-change-icon-modification);
	}
	.status-dot-wrap.deleted {
		--file-dot-color: var(--clr-change-icon-deletion);
	}
	.status-dot-wrap.renamed {
		--file-dot-color: var(--clr-change-icon-rename);
	}

	/* FULL-LARGE VARIANT */
	:global(.full-large) {
		height: var(--size-tag);
	}
</style>
