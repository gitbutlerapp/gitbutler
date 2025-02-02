<script lang="ts">
	type Props = {
		status: 'approved' | 'changes-requested' | 'unreviewed' | 'in-discussion';
	};

	const { status = 'unreviewed' }: Props = $props();
</script>

<div
	class="status-badge"
	class:status-badge_approved={status === 'approved'}
	class:status-badge_changes-requested={status === 'changes-requested'}
	class:status-badge_in-discussion={status === 'in-discussion'}
	class:status-badge_unreviewed={status === 'unreviewed'}
>
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
</style>
