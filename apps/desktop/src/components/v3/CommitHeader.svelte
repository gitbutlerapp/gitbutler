<script lang="ts">
	import { splitMessage } from '$lib/utils/commitMessage';

	type Props = {
		row?: boolean;
		commitMessage: string;
		className?: string;
	};

	const { commitMessage, row, className }: Props = $props();

	const title = $derived(splitMessage(commitMessage).title);
</script>

<h3 class="{className} commit-title" class:row>
	{title}
</h3>

<style>
	.row {
		text-align: left;
		text-overflow: ellipsis;
		overflow: hidden;
		white-space: nowrap;
	}
</style>
