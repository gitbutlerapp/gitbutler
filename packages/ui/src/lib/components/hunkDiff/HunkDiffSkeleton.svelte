<script lang="ts">
	interface Props {
		lineCount?: number;
	}

	const { lineCount = 3 }: Props = $props();
</script>

<div class="hunk-loading-skeleton">
	<div class="skeleton-header">
		<div class="skeleton-checkbox"></div>
		<div class="skeleton-hunk-info"></div>
	</div>
	<div class="skeleton-body">
		{#each Array(lineCount) as _}
			<div class="skeleton-line">
				<div class="skeleton-line-number"></div>
				<div class="skeleton-line-number"></div>
				<div class="skeleton-line-content" style="--width: {20 + Math.random() * 70}%"></div>
			</div>
		{/each}
	</div>
</div>

<style lang="postcss">
	.hunk-loading-skeleton {
		display: flex;
		flex-grow: 1;
		flex-direction: column;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background-color: var(--clr-diff-line-bg);
		font-family: var(--font-mono);
	}

	.skeleton-header {
		display: flex;
		align-items: center;
		height: 28px;
		border-bottom: 1px solid var(--clr-border-2);
		background-color: var(--clr-diff-count-bg);
	}

	.skeleton-checkbox {
		flex-shrink: 0;
		width: 60px;
		height: 16px;
		margin: 0 8px;
		border-radius: 3px;
		background-color: var(--clr-bg-2);
		opacity: 0.5;
	}

	.skeleton-hunk-info {
		flex: 1;
		height: 14px;
		margin: 0 8px;
		border-radius: 3px;
		background-color: var(--clr-bg-2);
		opacity: 0.5;
	}

	.skeleton-body {
		display: flex;
		flex-direction: column;
		padding: 8px 0;
	}

	.skeleton-line {
		display: flex;
		align-items: center;
		height: 20px;
		padding: 0 8px;
		gap: 8px;
	}

	.skeleton-line-number {
		flex-shrink: 0;
		width: 32px;
		height: 12px;
		border-radius: 3px;
		background-color: var(--clr-bg-2);
		opacity: 0.4;
	}

	.skeleton-line-content {
		width: var(--width);
		height: 12px;
		border-radius: 3px;
		background-color: var(--clr-bg-2);
		animation: skeleton-pulse 2s ease-in-out infinite;
		opacity: 0.5;
	}

	@keyframes skeleton-pulse {
		0%,
		100% {
			opacity: 0.5;
		}
		50% {
			opacity: 0.3;
		}
	}
</style>
