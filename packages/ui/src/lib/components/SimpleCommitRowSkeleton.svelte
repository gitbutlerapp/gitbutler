<script lang="ts">
	import SkeletonBone from "$components/SkeletonBone.svelte";

	interface Props {
		onlyContent?: boolean;
	}

	const { onlyContent }: Props = $props();
</script>

<div class="simple-commit-item no-select" class:content-only={onlyContent}>
	{#if !onlyContent}
		<SkeletonBone width="1rem" />
	{/if}
	<div class="content">
		<SkeletonBone maxWidth="80%" />
		<div class="details">
			<SkeletonBone height="0.75rem" width="3rem" />
			<SkeletonBone height="0.75rem" width="5rem" />
		</div>
	</div>
</div>

<style lang="postcss">
	.simple-commit-item {
		display: flex;
		overflow: hidden;
		gap: 10px;

		&:not(.content-only) {
			padding: 12px 14px 14px 12px;
			border-bottom: 1px solid var(--clr-border-2);

			&:last-child {
				border-bottom: none;
			}
		}

		& .content {
			display: flex;
			flex: 1;
			flex-direction: column;
			overflow: hidden;
			gap: 6px;
		}

		& .details {
			display: flex;
			align-items: center;
			overflow: hidden;
			gap: 4px;
		}
	}
</style>
