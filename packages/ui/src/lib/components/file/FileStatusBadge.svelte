<script lang="ts">
	import Badge from '$components/Badge.svelte';
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
				return 'neutral';
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
			<svg viewBox="0 0 11 11" fill="none" class="status-dot">
				<rect
					x="0.5"
					y="0.5"
					width="10"
					height="10"
					rx="3.5"
					stroke="var(--file-dot-color)"
					stroke-width="1.5"
				/>
				{#if status === 'addition'}
					<path d="M8.5 5.5H2.5M5.5 2.5V8.5" />
				{:else if status === 'modification'}
					<path d="M7.2626 3.73755L3.7374 7.26276" />
				{:else if status === 'deletion'}
					<path d="M8 5.5H3" />
				{:else if status === 'rename'}
					<path d="M7.5 5.5H0.5M7.5 5.5L4.5 2.5M7.5 5.5L4.5 8.5" />
				{/if}
			</svg>
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
	}

	.status-dot {
		width: 11px;
		height: 11px;
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

	/* FULL-LARGE VARIANT */
	:global(.full-large) {
		height: var(--size-tag);
	}
</style>
