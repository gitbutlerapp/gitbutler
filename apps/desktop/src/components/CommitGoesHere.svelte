<script lang="ts">
	import { Badge, TestId } from '@gitbutler/ui';

	type Props = {
		commitId: string | undefined;
		first?: boolean;
		last?: boolean;
		draft?: boolean;
		selected: boolean;
		onclick?: () => void;
	};

	const { commitId, first, last, draft, selected, onclick }: Props = $props();
</script>

{#snippet indicator(args?: { last?: boolean; first?: boolean; draft?: boolean })}
	<div
		data-testid={TestId.YourCommitGoesHere}
		data-testid-commit-id={commitId}
		class="indicator"
		class:first={args?.first}
		class:last={args?.last}
		class:draft={args?.draft}
	>
		<div class="pin">
			<div class="pin__line"></div>
			<div class="pin__circle"></div>
		</div>
		<div class="indicator__label waving-animation">
			<Badge size="tag" style="pop">Your commit goes here</Badge>
		</div>
	</div>
{/snippet}
{#snippet commitHere(args: { last?: boolean })}
	{@const last = args?.last}
	<button
		data-testid={TestId.CommitHereButton}
		data-testid-commit-id={commitId}
		class="commit-here"
		class:commit-here_first={first}
		class:commit-here_last={last}
		type="button"
		{onclick}
	>
		<div class="commit-here__line"></div>
		<div class="commit-here__circle"></div>

		<div class="commit-here__label text-11 text-semibold">Commit here</div>
	</button>
{/snippet}

{#if selected}
	{@render indicator({ first, last, draft })}
{/if}
{#if !selected}
	{@render commitHere({ last })}
{/if}

<style lang="postcss">
	.indicator {
		display: flex;
		align-items: center;
		padding: 12px 0;
		border-bottom: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-1);

		&.last {
			border-top: 1px solid var(--clr-border-2);
			border-bottom: none;
		}

		&.draft {
			border-top: 1px solid var(--clr-border-2);
			border-bottom: none;
		}
	}

	.pin {
		display: flex;
		position: relative;
		align-items: center;
		justify-content: center;
		width: 42px;
		height: 8px;
	}
	.pin__circle {
		position: relative;
		width: 8px;
		height: 8px;
		border-radius: 100%;
		outline: 2px solid var(--clr-theme-pop-element);
		background-color: var(--clr-bg-1);
	}
	.pin__line {
		position: absolute;
		top: 50%;
		left: 50%;
		width: 2px;
		height: 350%;
		transform: translate(-50%, -50%);
		background-color: var(--clr-theme-pop-element);
	}

	.indicator__label {
		display: flex;
	}

	/* COMMIT HERE */
	.commit-here {
		display: flex;
		z-index: var(--z-lifted);
		position: relative;
		align-items: center;
		width: 100%;
		height: 20px;
		margin-top: -10px;
		margin-bottom: -10px;
		background-color: var(--clr-bg-1);
		opacity: 0;
		transition: height var(--transition-medium);

		&:hover {
			opacity: 1;

			& .commit-here__label {
				transform: translateY(-50%) translateX(0) scale(1);
				opacity: 1;
			}

			&.commit-here_first {
				height: 30px;
				margin-top: 0;
			}

			&.commit-here_last {
				height: 30px;
				margin-bottom: 0;
			}
		}
	}
	.commit-here__circle {
		position: absolute;
		top: 50%;
		left: 16px;
		width: 10px;
		height: 10px;
		transform: translateY(-50%);
		border-radius: 100%;
		outline: 2px solid var(--clr-bg-1);
		background-color: var(--clr-theme-pop-element);
	}
	.commit-here__line {
		flex-grow: 1;
		height: 2px;
		margin-left: -15px;
		background-color: var(--clr-theme-pop-element);
	}
	.commit-here__label {
		position: absolute;
		top: 50%;
		left: 38px;
		padding: 3px 6px;
		transform: translateY(-50%) translateX(10%) scale(0.95);
		border-radius: var(--radius-ml);
		background-color: var(--clr-theme-pop-element);
		color: var(--clr-core-ntrl-100);
		transition:
			opacity var(--transition-fast),
			transform var(--transition-medium);
	}

	.waving-animation {
		animation: waving-animation 0.3s forwards;
	}
	@keyframes waving-animation {
		0% {
			transform: translateX(-3px);
		}
		50% {
			transform: translateX(2px);
		}
		100% {
			transform: translateX(0);
		}
	}
</style>
