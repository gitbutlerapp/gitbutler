<script lang="ts">
	import Badge from '$lib/Badge.svelte';
	import Tooltip from '$lib/Tooltip.svelte';
	import type { FileStatus } from '$lib/file/types';
	import type { ComponentColorType } from '$lib/utils/colorTypes';

	interface Props {
		status: FileStatus;
		style?: 'dot' | 'full';
		tooltip?: string;
	}

	const { status, style = 'full', tooltip }: Props = $props();

	const TOOLTIP_MAX_WIDTH = 320;

	function getFullStatusText(status: FileStatus): string {
		switch (status) {
			case 'A':
				return 'Added';
			case 'M':
				return 'Modified';
			case 'D':
				return 'Deleted';
			case 'R':
				return 'Renamed';
			default:
				return status;
		}
	}

	function getStatusColor(status: FileStatus): ComponentColorType {
		switch (status) {
			case 'A':
				return 'success';
			case 'M':
				return 'warning';
			case 'D':
				return 'error';
			case 'R':
				return 'purple';
			default:
				return 'neutral';
		}
	}
</script>

{#if style === 'dot'}
	<Tooltip text={!tooltip ? getFullStatusText(status) : tooltip} maxWidth={TOOLTIP_MAX_WIDTH}>
		<div
			class="status-dot-wrap"
			class:added={status === 'A'}
			class:modified={status === 'M'}
			class:deleted={status === 'D'}
			class:renamed={status === 'R'}
		>
			<svg width="11" height="11" viewBox="0 0 11 11" fill="none" class="status-dot">
				{#if status === 'A'}
					<path d="M9 5.5H2M5.5 2V9" />
				{:else if status === 'M'}
					<path d="M8 3.5L3 7.5" />
				{:else if status === 'D'}
					<path d="M8.5 5.5H2.5" />
				{:else if status === 'R'}
					<path d="M7.5 5.5H0.5M7.5 5.5L4.5 2.5M7.5 5.5L4.5 8.5" />
				{/if}
			</svg>
		</div>
	</Tooltip>
{:else if style === 'full'}
	<Tooltip text={status === 'R' && tooltip ? tooltip : undefined} maxWidth={TOOLTIP_MAX_WIDTH}>
		<Badge style={getStatusColor(status)} kind="soft">{getFullStatusText(status)}</Badge>
	</Tooltip>
{/if}

<style lang="postcss">
	.status-dot-wrap {
		width: fit-content;
		padding: 2px;
	}

	.status-dot {
		display: flex;
		flex-shrink: 0;
		border-radius: var(--radius-s);
		box-shadow: inset 0 0 0 1px var(--file-dot-color);
	}

	.status-dot path {
		stroke: var(--file-dot-color);
		stroke-width: 1.5;
	}

	/* MODIFIERS */
	.status-dot-wrap.added {
		--file-dot-color: var(--clr-scale-succ-60);
	}
	.status-dot-wrap.modified {
		--file-dot-color: var(--clr-scale-warn-60);
	}
	.status-dot-wrap.deleted {
		--file-dot-color: var(--clr-scale-err-60);
	}
	.status-dot-wrap.renamed {
		--file-dot-color: var(--clr-scale-purp-60);
	}
</style>
