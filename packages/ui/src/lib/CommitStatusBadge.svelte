<script lang="ts">
	import Icon from '$lib/Icon.svelte';

	type Props = {
		status: 'unreviewed' | 'in-discussion' | 'approved' | 'changes-requested';
		kind?: 'icon' | 'text';
	};

	const { status = 'unreviewed', kind = 'text' }: Props = $props();

	function getIconName() {
		if (status === 'approved') {
			return 'tick-small';
		} else if (status === 'changes-requested') {
			return 'refresh-small';
		} else if (status === 'in-discussion') {
			return 'dialog-small';
		} else {
			return 'minus-small';
		}
	}
</script>

<div
	class="status-badge"
	class:status-badge_icon={kind === 'icon'}
	class:status-badge_approved={status === 'approved'}
	class:status-badge_changes-requested={status === 'changes-requested'}
	class:status-badge_in-discussion={status === 'in-discussion'}
	class:status-badge_unreviewed={status === 'unreviewed'}
>
	{#if kind === 'icon'}
		<Icon name={getIconName()} />
	{:else}
		<span class="text-10 text-bold status-badge__text">
			{#if status === 'changes-requested'}
				Changes requested
			{:else if status === 'approved'}
				Approved
			{:else if status === 'in-discussion'}
				In discussion
			{:else}
				Unreviewed
			{/if}
		</span>
	{/if}
</div>

<style lang="postcss">
	.status-badge {
		display: flex;
		align-items: center;
		justify-content: center;
		border-radius: 100px;
		width: fit-content;
		height: var(--size-icon);
		text-wrap: nowrap;
		padding: 0 4px;
	}

	.status-badge_approved {
		background-color: var(--clr-br-commit-approved-bg);
		color: var(--clr-br-commit-approved-text);
	}

	.status-badge_changes-requested {
		background-color: var(--clr-br-commit-changes-requested-bg);
		color: var(--clr-br-commit-changes-requested-text);
	}

	.status-badge_in-discussion {
		background-color: var(--clr-br-commit-in-discussion-bg);
		color: var(--clr-br-commit-in-discussion-text);
	}

	.status-badge_unreviewed {
		background-color: var(--clr-br-commit-unreviewed-bg);
		color: var(--clr-br-commit-unreviewed-text);
	}

	.status-badge_icon {
		flex-shrink: 0;
		width: var(--size-icon);
		max-width: var(--size-icon);
	}
</style>
