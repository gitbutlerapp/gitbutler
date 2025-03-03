<script lang="ts">
	import { getColorFromBranchType } from '$components/v3/lib';

	interface Props {
		lastBranch?: boolean;
		selected?: boolean;
	}

	const { lastBranch, selected }: Props = $props();
</script>

<div class="empty-series" style:--commit-color={getColorFromBranchType('LocalOnly')}>
	<div class="commit-line" class:selected class:dashed={lastBranch}></div>
	<div class="text-13 text-body empty-series__label">
		This is an empty branch.
		<br />
		Create or drag & drop commits here
	</div>
</div>

<style lang="postcss">
	.empty-series {
		position: relative;
		display: flex;
		border-top: 1px solid var(--clr-border-2);
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
	:global(.commit-goes-here .empty-series .commit-line) {
		opacity: 0.4;
	}
</style>
