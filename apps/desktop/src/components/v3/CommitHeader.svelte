<script lang="ts">
	import { TestId } from '$lib/testing/testIds';
	import { splitMessage } from '$lib/utils/commitMessage';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';

	type Props = {
		truncate?: boolean;
		commitMessage: string;
		className?: string;
	};

	const { commitMessage, truncate, className }: Props = $props();

	const title = $derived(splitMessage(commitMessage).title);
</script>

<Tooltip text={title}>
	<h3 data-testid={TestId.CommitDrawerTitle} class="{className} commit-title" class:truncate>
		{title}
	</h3>
</Tooltip>
