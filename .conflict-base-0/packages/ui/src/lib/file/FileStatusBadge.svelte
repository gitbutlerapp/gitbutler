<script lang="ts">
	import Badge from '$lib/Badge.svelte';
	import Tooltip from '$lib/Tooltip.svelte';
	import type { ComponentColorType } from '$lib/utils/colorTypes';
	import type { FileStatus } from './types';

	interface Props {
		status: FileStatus;
		style?: 'dot' | 'full';
	}

	const { status, style = 'full' }: Props = $props();

	function getFullStatusText(status: FileStatus): string {
		switch (status) {
			case 'A':
				return 'Added';
			case 'M':
				return 'Modified';
			case 'D':
				return 'Deleted';
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
			default:
				return 'neutral';
		}
	}
</script>

{#if style === 'dot'}
	<Tooltip text={getFullStatusText(status)}>
		<div class="status-dot-wrap">
			<div
				class="status-dot"
				class:added={status === 'A'}
				class:modified={status === 'M'}
				class:deleted={status === 'D'}
			></div>

			<svg
				width="10"
				height="10"
				viewBox="0 0 10 10"
				fill="none"
				xmlns="http://www.w3.org/2000/svg"
				class="status-dot"
				class:added={status === 'A'}
				class:modified={status === 'M'}
				class:deleted={status === 'D'}
			>
				<path
					d="M3 0.75H7C8.24264 0.75 9.25 1.75736 9.25 3V7C9.25 8.24264 8.24264 9.25 7 9.25H3C1.75736 9.25 0.75 8.24264 0.75 7V3C0.75 1.75736 1.75736 0.75 3 0.75Z"
					stroke-width="1.5"
				/>
				{#if status === 'A'}
					<path d="M7.75 5H2.25M5 2.25V7.75" stroke-width="1.5" />
				{:else if status === 'M'}
					<path d="M6 4L4 6" stroke-width="2" />
				{:else if status === 'D'}
					<path d="M7.5 5H2.5" stroke-width="2" />
				{/if}
			</svg>
		</div>
	</Tooltip>
{:else if style === 'full'}
	<Badge style={getStatusColor(status)}>{getFullStatusText(status)}</Badge>
{/if}

<style lang="postcss">
	.status-dot-wrap {
		display: flex;
		align-items: center;
		justify-content: center;
		min-width: 16px;
	}

	.status-dot {
		flex-shrink: 0;
	}
	.status-dot.added path {
		stroke: var(--clr-scale-succ-60);
	}
	.status-dot.modified path {
		stroke: var(--clr-scale-warn-60);
	}
	.status-dot.deleted path {
		stroke: var(--clr-scale-err-60);
	}
</style>
