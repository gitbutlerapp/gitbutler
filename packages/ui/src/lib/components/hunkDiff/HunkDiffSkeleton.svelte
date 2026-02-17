<script lang="ts">
	import SkeletonBone from "$components/SkeletonBone.svelte";
</script>

<div class="hunk-loading-skeleton">
	<div class="skeleton-header">
		<SkeletonBone width="32px" height="12px" />
		<SkeletonBone width="32px" height="12px" />
		<SkeletonBone width="100px" height="12px" />
	</div>
	<div class="skeleton-body">
		{#each Array(3) as _, i}
			<div class="skeleton-line" style:opacity={1 - i * 0.23}>
				<SkeletonBone width="32px" height="12px" />
				<SkeletonBone width="32px" height="12px" />
				<SkeletonBone width="{20 + Math.random() * 70}%" height="12px" />
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
		padding: 0 10px;
		gap: 8px;
		border-bottom: 1px solid var(--clr-border-2);
		background-color: var(--clr-diff-count-bg);
	}

	.skeleton-body {
		display: flex;
		flex-direction: column;
		padding: 10px;
		gap: 8px;
	}

	.skeleton-line {
		display: flex;
		align-items: center;
		gap: 8px;
	}
</style>
