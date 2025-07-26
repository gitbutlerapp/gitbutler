<script lang="ts" module>
	export type CommitStatusType =
		| 'unreviewed'
		| 'in-discussion'
		| 'approved'
		| 'changes-requested'
		| 'closed'
		| 'loading';
</script>

<script lang="ts">
	import Icon from '$components/Icon.svelte';

	type Props = {
		status: CommitStatusType;
		kind?: 'icon' | 'text' | 'both';
		lineTop?: boolean;
		lineBottom?: boolean;
	};

	const {
		status = 'unreviewed',
		kind = 'text',
		lineTop = false,
		lineBottom = false
	}: Props = $props();

	function getIconName() {
		if (status === 'approved') {
			return 'tick-small';
		} else if (status === 'changes-requested') {
			return 'refresh-small';
		} else if (status === 'in-discussion') {
			return 'dialog-small';
		} else if (status === 'closed') {
			return 'cross-small';
		} else {
			return 'dotted-circle';
		}
	}

	function statusClasses(type: 'icon' | 'text') {
		return {
			'status-badge': true,
			'status-badge_icon': type === 'icon',
			'status-badge_approved': status === 'approved',
			'status-badge_closed': status === 'closed',
			'status-badge_loading': status === 'loading',
			'status-badge_changes-requested': status === 'changes-requested',
			'status-badge_in-discussion': status === 'in-discussion',
			'status-badge_unreviewed': status === 'unreviewed'
		};
	}
</script>

{#snippet icon()}
	<div class={statusClasses('icon')}>
		<Icon name={getIconName()} />
	</div>
{/snippet}

{#snippet text()}
	<div class={statusClasses('text')}>
		<span class="text-10 text-bold status-badge__text">
			{#if status === 'closed'}
				Closed
			{:else if status === 'loading'}
				Processing
			{:else if status === 'changes-requested'}
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
{/snippet}

{#if lineTop || lineBottom}
	<div class="has-lines">
		<div class="line-container line-top" class:line-visible={lineTop}></div>
		<div class="status-badges">
			{#if kind === 'icon' || kind === 'both'}
				{@render icon()}
			{/if}

			{#if kind === 'text' || kind === 'both'}
				{@render text()}
			{/if}
		</div>
		<div class="line-container line-bottom" class:line-visible={lineBottom}></div>
	</div>
{:else}
	<div class="status-badges">
		{#if kind === 'icon' || kind === 'both'}
			{@render icon()}
		{/if}

		{#if kind === 'text' || kind === 'both'}
			{@render text()}
		{/if}
	</div>
{/if}

<style lang="postcss">
	.line-container {
		flex-grow: 1;
	}

	.line-visible {
		&::after {
			display: block;
			width: 8px;
			height: calc(100% - 4px);
			border-right: 1px solid var(--clr-border-2);
			content: '';
		}
	}
	.line-visible.line-bottom {
		&::after {
			transform: translateY(4px);
		}
	}

	.has-lines {
		display: flex;
		flex-direction: column;
		height: 100%;
	}

	.status-badges {
		display: flex;
		gap: 10px;
	}

	.status-badge {
		display: flex;
		align-items: center;
		justify-content: center;
		width: fit-content;
		height: var(--size-icon);
		padding: 0 5px;
		border-radius: 100px;
		text-wrap: nowrap;
	}
	.status-badge_closed {
		background-color: var(--clr-br-commit-closed-bg);
		color: var(--clr-br-commit-closed-text);
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

	.status-badge_unreviewed,
	.status-badge_loading {
		background-color: var(--clr-br-commit-unreviewed-bg);
		color: var(--clr-br-commit-unreviewed-text);
	}

	.status-badge_icon {
		flex-shrink: 0;
		width: var(--size-icon);
		max-width: var(--size-icon);
	}
</style>
