<script lang="ts">
	import { getBranchStatusLabelAndColor } from '$components/lib';
	import type { PushStatus } from '$lib/stacks/stack';

	type Props = {
		pushStatus: PushStatus;
		unstyled?: boolean;
	};

	const { pushStatus, unstyled }: Props = $props();

	const [label, bgColor] = $derived.by((): [string, string] => {
		const { label, color } = getBranchStatusLabelAndColor(pushStatus);
		return [label, color];
	});
</script>

<span
	class={[!unstyled && 'text-11 text-bold branch-badge', 'truncate']}
	style:--b-bg-color={bgColor}
>
	{label}
</span>

<style class="postcss">
	.branch-badge {
		display: flex;
		align-items: center;
		justify-content: center;
		height: var(--size-icon);
		padding: 0 5px;
		border-radius: 20px;
		background-color: var(--b-bg-color);
		color: #fff;
		line-height: 1;
		text-align: center;
	}
</style>
