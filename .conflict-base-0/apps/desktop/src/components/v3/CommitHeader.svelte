<script lang="ts">
	import { TestId } from '$lib/testing/testIds';
	import { splitMessage } from '$lib/utils/commitMessage';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';

	type Props = {
		row?: boolean;
		commitMessage: string;
		className?: string;
	};

	const { commitMessage, row, className }: Props = $props();

	const title = $derived(splitMessage(commitMessage).title);
</script>

<Tooltip text={title}>
	<h3 data-testid={TestId.CommitDrawerTitle} class="{className} commit-title" class:row>
		{title}
	</h3>
</Tooltip>

<style>
	.row {
		text-align: left;
		text-overflow: ellipsis;
		overflow: hidden;
		white-space: nowrap;
	}
</style>
