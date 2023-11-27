<script lang="ts">
	export let status: 'A' | 'M' | 'D';
</script>

<div
	class="status"
	class:added={status == 'A'}
	class:modified={status == 'M'}
	class:deleted={status == 'D'}
></div>

<style lang="postcss">
	.status {
		border-radius: 100%;
		width: var(--space-8);
		height: var(--space-8);
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
