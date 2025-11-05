<script lang="ts">
	import { type Snippet } from 'svelte';

	interface Props {
		testId?: string;
		isLast?: boolean;
		action: Snippet;
	}

	const { isLast, action, testId }: Props = $props();
</script>

<div data-testid={testId} class="action-row" class:is-last={isLast}>
	<div class="commit-line-wrapper">
		<div class="commit-line" class:dashed={isLast}></div>
	</div>

	<div class="action">
		{@render action()}
	</div>
</div>

<style lang="postcss">
	.action-row {
		display: flex;
		position: relative;
		overflow: hidden;
		border-top: 1px solid var(--clr-border-3);
		background-color: var(--clr-theme-warn-bg);
		--commit-color: var(--clr-commit-upstream);

		&:not(.is-last) {
			border-bottom: 1px solid var(--clr-border-2);
		}
	}

	.action {
		display: flex;
		flex-direction: column;
		width: 100%;
		padding-top: 14px;
		padding-right: 14px;
		padding-bottom: 14px;
	}

	.commit-line-wrapper {
		position: relative;
		margin-right: 20px;
		margin-left: 20px;
	}
</style>
