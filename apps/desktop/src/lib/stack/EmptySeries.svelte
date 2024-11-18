<script lang="ts">
	import { getColorFromBranchType } from '@gitbutler/ui/utils/getColorFromBranchType';
	interface Props {
		isBottom?: boolean;
	}

	const { isBottom }: Props = $props();
</script>

<div class="empty-series" style:--commit-color={getColorFromBranchType('local')}>
	<div class="commit-line" class:dashed={isBottom}></div>
	<div class="text-13 text-body empty-series__label">
		This is an empty branch.
		<br />
		Create or drag & drop commits here
	</div>
</div>

<style lang="postcss">
	.empty-series {
		display: flex;
	}
	.empty-series__label {
		color: var(--clr-text-2);

		width: 100%;
		padding: 20px 28px 26px 46px;
		opacity: 0.6;
	}
	.commit-line {
		position: absolute;
		left: 20px;
	}
</style>
