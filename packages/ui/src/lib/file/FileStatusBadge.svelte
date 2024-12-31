<script lang="ts">
	import Badge from '$lib/Badge.svelte';
	import type { ComponentColor, ComponentStyleKind } from '$lib/utils/colorTypes';
	import type { FileStatus } from './types';

	interface Props {
		status: FileStatus;
		style?: 'dot' | 'full';
		kind?: ComponentStyleKind;
	}

	const { status, style = 'full', kind = 'solid' }: Props = $props();

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

	function getStatusColor(status: FileStatus): ComponentColor {
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
	<div class="status-dot-wrap">
		<div
			class="status-dot"
			class:added={status === 'A'}
			class:modified={status === 'M'}
			class:deleted={status === 'D'}
		></div>
	</div>
{:else if style === 'full'}
	<Badge style={getStatusColor(status)} {kind}>{getFullStatusText(status)}</Badge>
{/if}

<style lang="postcss">
	.status-dot-wrap {
		display: flex;
		align-items: center;
		justify-content: center;
		min-width: 16px;
	}

	.status-dot {
		border-radius: 100%;
		width: 8px;
		height: 8px;
		border-radius: 100%;
		flex-shrink: 0;
	}
	.status-dot.added {
		background: var(--clr-scale-succ-60);
	}
	.status-dot.modified {
		background: var(--clr-scale-warn-60);
		opacity: 0.6;
	}
	.status-dot.deleted {
		background: var(--clr-scale-err-50);
		opacity: 0.8;
	}
</style>
