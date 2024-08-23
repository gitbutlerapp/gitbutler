<script lang="ts">
	import type { FileStatus } from './types';

	interface Props {
		status: FileStatus;
	}

	const { status }: Props = $props();
</script>

<div class="status-wrap">
	<div
		class="status-dot"
		class:added={status === 'A'}
		class:modified={status === 'M'}
		class:deleted={status === 'D'}
	></div>
</div>

<style lang="postcss">
	.status-wrap {
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
		opacity: 0.6;
	}
	.status-dot.added {
		background: var(--clr-scale-succ-50);
	}
	.status-dot.modified {
		background: var(--clr-scale-warn-50);
	}
	.status-dot.deleted {
		background: var(--clr-scale-err-50);
	}
</style>
