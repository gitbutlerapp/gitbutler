<script lang="ts">
	import { splitMessage } from '$lib/utils/commitMessage';
	import { TestId, Tooltip } from '@gitbutler/ui';

	type Props = {
		truncate?: boolean;
		commitMessage: string;
		className?: string;
		editable?: boolean;
		onclick?: () => void;
	};

	const { commitMessage, truncate, className, editable, onclick }: Props = $props();

	const title = $derived(splitMessage(commitMessage).title);

	function getTitle() {
		if (title) {
			return title;
		}
		return editable ? 'Empty commit. Drag changes here' : 'Empty commit';
	}
</script>

<Tooltip text={getTitle()} delay={1200}>
	<h3
		data-testid={TestId.CommitDrawerTitle}
		class="{className} commit-title"
		class:truncate
		class:clr-text-3={!title}
		class:clickable={editable && onclick}
		onclick={editable && onclick ? onclick : undefined}
		role={editable && onclick ? 'button' : undefined}
		tabindex={editable && onclick ? 0 : undefined}
		onkeydown={editable && onclick
			? (e) => {
					if (e.key === 'Enter' || e.key === ' ') {
						e.preventDefault();
						onclick();
					}
				}
			: undefined}
	>
		{getTitle()}
	</h3>
</Tooltip>

<style>
	.commit-title.clickable {
		cursor: text;
	}

	.commit-title.clickable:hover {
		opacity: 0.8;
	}
</style>
