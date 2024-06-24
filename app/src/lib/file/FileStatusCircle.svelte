<script lang="ts">
	import type { FileStatus } from '$lib/utils/fileStatus';

	export let status: FileStatus;
</script>

<div
	class="status"
	class:added={status === 'A'}
	class:modified={status === 'M'}
	class:deleted={status === 'D'}
></div>

<style lang="postcss">
	.status {
		border-radius: 100%;
		width: 8px;
		height: 8px;
		border-radius: 100%;
		flex-shrink: 0;
	}
	.added {
		background: rgba(101, 199, 154, 0.5);
	}
	.modified {
		background: rgba(255, 184, 46, 0.5);
	}
	.deleted {
		background: rgba(255, 61, 78, 0.5);
	}
</style>
