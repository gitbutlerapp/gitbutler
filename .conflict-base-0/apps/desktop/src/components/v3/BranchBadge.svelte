<script lang="ts">
	import { getBranchStatusLabelAndColor } from '$components/v3/lib';
	import type { PushStatus } from '$lib/stacks/stack';
	import type { ComponentColorType } from '@gitbutler/ui/utils/colorTypes';

	type Props = {
		pushStatus: PushStatus;
		unstyled?: boolean;
	};

	const { pushStatus, unstyled }: Props = $props();

	const [label, bgColor] = $derived.by((): [string, ComponentColorType] => {
		const { label, color } = getBranchStatusLabelAndColor(pushStatus);
		return [label, color];
	});
</script>

<span class={[!unstyled && 'text-10 text-bold branch-badge truncate']} style:--b-bg-color={bgColor}>
	<span class:truncate={!unstyled}>{label}</span>
</span>

<style class="postcss">
	.branch-badge {
		display: flex;
		align-items: center;
		justify-content: center;
		text-align: center;
		border-radius: 20px;
		color: #fff;
		background-color: var(--b-bg-color);
		padding: 0 5px;
		height: var(--size-icon);
		line-height: 1;
	}
</style>
