<script lang="ts">
	import { splitMessage } from '$lib/utils/commitMessage';
	import { TestId, Tooltip } from '@gitbutler/ui';

	type Props = {
		truncate?: boolean;
		commitMessage: string;
		className?: string;
	};

	const { commitMessage, truncate, className }: Props = $props();

	const title = $derived(splitMessage(commitMessage).title);

	function getTitle() {
		if (title) {
			return title;
		}
		return 'Empty commit. Drag changes here';
	}
</script>

<Tooltip text={getTitle()}>
	<h3
		data-testid={TestId.CommitDrawerTitle}
		class="{className} commit-title"
		class:truncate
		class:clr-text-3={!title}
	>
		{getTitle()}
	</h3>
</Tooltip>
