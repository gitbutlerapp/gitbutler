<script lang="ts">
	import Tooltip from '$lib/Tooltip.svelte';
	import FileStatusBadge from '$lib/file/FileStatusBadge.svelte';
	import type { FileStatus } from '$lib/file/types';

	interface Props {
		status?: FileStatus;
		added?: number;
		removed?: number;
	}

	const { status, added, removed }: Props = $props();
</script>

<div class="file-stats">
	<Tooltip text="Lines added/removed" delay={1200}>
		<div class="file-stats__lines text-11 text-bold">
			{#if added}
				<span class="file-stats__lines--added">+{added}</span>
			{/if}
			{#if removed}
				<span class="file-stats__lines--removed">-{removed}</span>
			{/if}
		</div>
	</Tooltip>

	{#if status}
		<FileStatusBadge {status} style="full" />
	{/if}
</div>

<style lang="postcss">
	.file-stats {
		display: flex;
		align-items: center;
		gap: 12px;
	}

	.file-stats__lines {
		display: flex;
		align-items: center;
		gap: 4px;
	}

	.file-stats__lines--added {
		color: var(--clr-theme-succ-element);
	}

	.file-stats__lines--removed {
		color: var(--clr-theme-err-element);
	}
</style>
