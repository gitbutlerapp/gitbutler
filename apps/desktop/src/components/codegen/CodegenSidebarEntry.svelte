<script lang="ts">
	import { Badge, Icon, TimeAgo, Tooltip, InfoButton } from '@gitbutler/ui';
	import { focusable } from '@gitbutler/ui/focus/focusable';
	import { slide, fade } from 'svelte/transition';
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
		totalHeads: number;
		sessionInGui?: boolean;
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
		branchIcon,
		totalHeads,
		sessionInGui
	}: Props = $props();

	let isOpen = $state(false);
</script>

<div class="codegen-entry-wrapper" use:focusable>
	<div class="codegen-entry" class:disabled={sessionInGui === false}>
		{#if sessionInGui !== false}
			<button class="codegen-entry-header" class:selected type="button" {onclick}>
				{@render headerContent()}
			</button>
		{:else}
			<button class="codegen-entry-header" class:selected type="button" disabled>
				{@render headerContent({
					disabled: true
				})}
				<p class="text-12 text-body clr-text-2">
					⚠ This session was created via CLI and can't be continued in the GUI yet.
				</p>
			</button>
		{/if}

		{#if commitCount > 0}
			<div class="commits-drawer">
				<button class="commits-drawer-header" onclick={() => (isOpen = !isOpen)} type="button">
					<div class="fold-icon" class:open={isOpen}>
						<Icon name="chevron-right" />
					</div>
					<p class="text-13 text-semibold">Commits</p>
					<Badge kind="soft">{commitCount}</Badge>
				</button>

				{#if isOpen}
					<div class="stack-v full-width" transition:slide|local={{ duration: 150, axis: 'y' }}>
						<div class="commits-list">
							{@render commits()}
						</div>
					</div>
				{/if}
			</div>
		{/if}
	</div>

	{#if totalHeads > 1}
		<div class="entry-heads">
			<Icon name="stack" color="var(--clr-text-3)" />
			<p class="text-12 text-semibold full-width">{totalHeads - 1} more branches in stack</p>

			<InfoButton>
				Currently GitButler doesn’t support multiple sessions for a stack. A session is applied only
				to the top branch.
			</InfoButton>
		</div>
	{/if}
</div>

{#snippet headerContent(props = { disabled: false })}
	{#if selected}
		<div class="active-indicator" in:slide={{ axis: 'x', duration: 150 }}></div>
	{/if}
	<div class="entry-header-content">
		{@render branchIcon()}
		<p class="text-14 text-bold truncate full-width">{branchName}</p>
		{@render vibeIcon()}
	</div>

	{#if !props.disabled}
		{#if tokensUsed || cost}
			<div class="entry-metadata text-12" in:fade={{ duration: 150 }}>
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
						<div class="metadata-divider"></div>
						<p>${cost.toFixed(2)}</p>
					</div>
				</Tooltip>

				{#if lastInteractionTime}
					<p class="text-11 last-interaction-time opacity-60">
						<TimeAgo date={lastInteractionTime} addSuffix />
					</p>
				{/if}
			</div>
		{/if}
	{/if}
{/snippet}

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
	.codegen-entry-wrapper {
		display: flex;
		flex-direction: column;
		align-items: center;
	}

	.codegen-entry {
		z-index: var(--z-ground);
		position: relative;
		flex-shrink: 0;
		width: 100%;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		background-color: var(--clr-bg-1);

		&.disabled {
			background-color: var(--clr-bg-1-muted);
			opacity: 0.7;

			.codegen-entry-header {
				background-color: transparent;
				cursor: not-allowed;

				&:hover {
					background-color: transparent;
				}
			}
		}
	}

	.codegen-entry-header {
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

		&:not(.selected, &:disabled):hover {
			background-color: var(--clr-bg-1-muted);
		}

		&:disabled {
			background-color: var(--clr-bg-2);
			cursor: not-allowed;
		}
	}

	.entry-header-content {
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
	.commits-drawer {
		display: flex;
		flex-direction: column;
		width: 100%;
		border-top: 1px solid var(--clr-border-2);
	}

	.commits-drawer-header {
		display: flex;
		align-items: center;
		width: 100%;
		padding: 10px 12px;
		gap: 6px;
		background-color: var(--clr-bg-2);

		&:hover {
			.fold-icon {
				color: var(--clr-text-2);
			}
		}
	}

	.fold-icon {
		display: flex;
		color: var(--clr-text-3);
		transition:
			transform 0.15s ease-in-out,
			color var(--transition-fast);

		&.open {
			transform: rotate(90deg);
		}
	}

	.entry-metadata {
		display: flex;
		align-items: center;
		justify-content: space-between;
		width: 100%;
		padding-left: 2px;
		gap: 4px;
		opacity: 0.7;
	}

	.metadata-divider {
		width: 1px;
		height: 12px;
		margin: 0 4px;
		background-color: var(--clr-text-1);
		opacity: 0.3;
	}

	.commits-list {
		display: flex;
		flex-direction: column;
		border-top: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-1);
	}

	.vibe-icon {
		display: flex;

		&.completed {
			color: var(--clr-theme-succ-element);
		}
	}

	.active-indicator {
		position: absolute;
		top: 12px;
		left: 0;
		width: 12px;
		height: 18px;
		transform: translateX(-50%);
		border-radius: var(--radius-m);
		background-color: var(--clr-theme-pop-element);
	}

	.entry-heads {
		display: flex;
		position: relative;
		align-items: center;
		width: calc(100% - 8px);
		padding: 12px 12px 10px;
		gap: 8px;
		transform: translateY(-4px);
		border: 1px solid var(--clr-border-3);
		border-radius: 0 0 var(--radius-ml) var(--radius-ml);
		background: linear-gradient(180deg, var(--clr-bg-2) 20%, var(--clr-bg-1) 200%);
		background-color: var(--clr-bg-2);
		color: var(--clr-text-2);
	}
</style>
