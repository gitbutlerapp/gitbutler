<script lang="ts">
	import { Badge, Icon, TimeAgo, Tooltip } from '@gitbutler/ui';
	import { slide } from 'svelte/transition';
	import type { ClaudeStatus } from '$lib/codegen/types';
	import type { Snippet } from 'svelte';

	type Props = {
		selected: boolean;
		status: ClaudeStatus;
		branchName: string;
		tokensUsed: number;
		cost: number;
		commitCount: number;
		lastInteractionTime?: Date;
		commits: Snippet;
		onclick: (e: MouseEvent) => void;
		branchIcon: Snippet;
	};

	const {
		selected,
		status,
		branchName,
		tokensUsed,
		cost,
		commitCount,
		lastInteractionTime,
		commits,
		onclick,
		branchIcon
	}: Props = $props();

	let isOpen = $state(false);
</script>

<div class="sidebar-entry">
	<button class="sidebar-entry-header" class:selected type="button" {onclick}>
		{#if selected}
			<div class="entry-active-indicator" in:slide={{ axis: 'x', duration: 150 }}></div>
		{/if}
		<div class="sidebar-entry-header-left">
			{@render branchIcon()}

			<p class="text-14 text-bold truncate full-width">{branchName}</p>
			{@render vibeIcon()}
		</div>

		{#if status !== 'disabled'}
			<div class="sidebar-entry-drawer__header-info text-12">
				<Tooltip text="Total tokens used and cost">
					<div class="flex gap-4 items-center">
						<p>{tokensUsed}</p>

						<svg
							width="0.938rem"
							height="0.938rem"
							viewBox="0 0 15 15"
							fill="none"
							xmlns="http://www.w3.org/2000/svg"
							opacity="0.6"
						>
							<circle cx="7.5" cy="7.5" r="5.5" stroke="currentColor" stroke-width="1.5" />
							<circle
								cx="7.50015"
								cy="7.5"
								r="2.92106"
								transform="rotate(-45 7.50015 7.5)"
								stroke="currentColor"
								stroke-width="1.5"
								stroke-dasharray="2 1"
							/>
						</svg>

						<div class="sidebar-entry-drawer__header-info__divider"></div>
						<p>${cost.toFixed(2)}</p>
					</div>
				</Tooltip>

				{#if lastInteractionTime}
					<p class="text-11 sidebar-entry-drawer__tima-ago opacity-60">
						<TimeAgo date={lastInteractionTime} addSuffix />
					</p>
				{/if}
			</div>
		{/if}
	</button>

	{#if commitCount > 0}
		<div class="sidebar-entry-drawer">
			<button class="sidebar-entry-drawer__header" onclick={() => (isOpen = !isOpen)} type="button">
				<div class="sidebar-entry-drawer__header__fold-icon" class:open={isOpen}>
					<Icon name="chevron-right" />
				</div>
				<p class="text-13 text-semibold">Commits</p>
				<Badge kind="soft">{commitCount}</Badge>
			</button>

			{#if isOpen}
				<div class="stack-v full-width" transition:slide|local={{ duration: 150, axis: 'y' }}>
					<div class="sidebar-entry-drawer__commits">
						{@render commits()}
					</div>
				</div>
			{/if}
		</div>
	{/if}
</div>

{#snippet vibeIcon()}
	<div class="vibe-icon {status}">
		{#if status === 'running'}
			<Icon name="spinner" />
		{:else if status === 'completed'}
			<Icon name="success" />
		{/if}
	</div>
{/snippet}

<style lang="postcss">
	.sidebar-entry {
		flex-shrink: 0;
		width: 100%;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
	}

	.sidebar-entry-header {
		display: flex;
		position: relative;
		flex-direction: column;
		width: 100%;
		padding: 12px;
		overflow: hidden;
		gap: 10px;
		text-align: left;

		&.selected {
			background-color: var(--clr-selected-in-focus-bg);
		}

		&:not(.selected):hover {
			background-color: var(--clr-bg-1-muted);
		}
	}

	.sidebar-entry-header-left {
		display: flex;
		align-items: center;
		width: 100%;
		overflow: hidden;
		gap: 10px;
	}

	.last-interaction {
		display: flex;
		padding: 8px 12px 6px 12px;
		border-top: 1px solid var(--clr-border-3);
		background-color: var(--clr-bg-1-muted);
		opacity: 0.8;
	}

	/* DRAWER */
	.sidebar-entry-drawer {
		display: flex;
		flex-direction: column;
		width: 100%;
		border-top: 1px solid var(--clr-border-2);
	}

	.sidebar-entry-drawer__header {
		display: flex;
		align-items: center;
		width: 100%;
		padding: 10px 12px;
		gap: 6px;
		background-color: color-mix(in srgb, var(--clr-bg-2) 60%, transparent);

		&:hover {
			.sidebar-entry-drawer__header__fold-icon {
				color: var(--clr-text-2);
			}
		}
	}

	.sidebar-entry-drawer__header__fold-icon {
		display: flex;
		color: var(--clr-text-3);
		transition:
			transform 0.15s ease-in-out,
			color var(--transition-fast);

		&.open {
			transform: rotate(90deg);
		}
	}

	.sidebar-entry-drawer__header-info {
		display: flex;
		align-items: center;
		justify-content: space-between;
		width: 100%;
		padding-left: 2px;
		gap: 4px;
		opacity: 0.7;
	}

	.sidebar-entry-drawer__header-info__divider {
		width: 1px;
		height: 12px;
		margin: 0 4px;
		background-color: var(--clr-text-1);
		opacity: 0.3;
	}

	.sidebar-entry-drawer__commits {
		display: flex;
		flex-direction: column;
		border-top: 1px solid var(--clr-border-2);
	}

	.vibe-icon {
		display: flex;

		&.enabled {
			/* color: var(--clr-theme-pop-element); */
		}
		&.running {
			/* color: var(--clr-theme-pop-element); */
		}
		&.completed {
			/* color: var(--clr-theme-succ-element); */
		}
	}

	.entry-active-indicator {
		position: absolute;
		top: 12px;
		left: 0;
		width: 12px;
		height: 18px;
		transform: translateX(-50%);
		border-radius: var(--radius-m);
		background-color: var(--clr-theme-pop-element);
	}
</style>
