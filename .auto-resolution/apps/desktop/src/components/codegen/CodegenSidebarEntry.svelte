<script lang="ts">
	import Drawer from '$components/Drawer.svelte';
	import { Badge, Icon } from '@gitbutler/ui';
	import type { Snippet } from 'svelte';

	type Props = {
		selected: boolean;
		status: 'no-vibes' | 'vibes' | 'running' | 'completed' | 'assistance-required';
		branchName: string;
		tokensUsed: number;
		cost: number;
		commitCount: number;

		commits: Snippet;
		onclick: (e: MouseEvent) => void;
	};

	const { selected, status, branchName, tokensUsed, cost, commitCount, commits, onclick }: Props =
		$props();
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<div class="sidebar-entry" {onclick}>
	<div class="sidebar-entry-header" class:selected>
		<div class="sidebar-entry-header-left">
			<Icon name="branch-remote" />
			<p class="text-13 text-semibold">{branchName}</p>
		</div>
		{@render vibeIcon()}
	</div>
	<Drawer children={commits} defaultCollapsed>
		{#snippet header()}
			<div class="sidebar-entry-drawer">
				<div class="flex gap-6 items-center">
					<p class="text-14 text-semibold">Commits</p>
					<Badge>{commitCount}</Badge>
				</div>
				<div class="flex gap-8 items-center">
					<p class="text-12">{tokensUsed}</p>
					<div class="iddy-biddy-line"></div>
					<p class="text-12">{cost.toFixed(2)}</p>
				</div>
			</div>
		{/snippet}
	</Drawer>
</div>

{#snippet vibeIcon()}
	<div class="vibe-icon {status}">
		{#if status === 'vibes'}
			<Icon name="ai" />
		{:else if status === 'running'}
			<Icon name="running-man" />
		{:else if status === 'completed'}
			<Icon name="success" />
		{:else if status === 'assistance-required'}
			<Icon name="error" />
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
		align-items: flex-start;
		align-self: stretch;
		justify-content: center;
		padding: 12px 12px 12px 14px;
		gap: 12px;

		border-bottom: 1px solid var(--clr-border-2);

		&.selected {
			background-color: var(--clr-selected-in-focus-bg);
			&::before {
				position: absolute;

				top: 50%;
				left: 0px;

				width: 6px;
				height: 21px;

				transform: translateY(-50%);

				border-radius: 0 10px 10px 0;

				background-color: var(--clr-selected-in-focus-element);
				content: '';
			}
		}
	}

	.sidebar-entry-header-left {
		display: flex;
		flex: 1 0 0;
		align-items: center;
		gap: 8px;
	}

	.sidebar-entry-drawer {
		display: flex;
		flex-grow: 1;

		align-items: center;
		justify-content: space-between;
	}

	.iddy-biddy-line {
		width: 1px;
		height: 11px;
		background-color: var(--clr-text-3);
	}

	.vibe-icon {
		&.vibes {
			color: var(--clr-theme-pop-element);
		}
		&.running {
			color: var(--clr-theme-pop-element);
		}
		&.completed {
			color: var(--clr-theme-succ-element);
		}
		&.assistance-required {
			color: var(--clr-theme-err-element);
		}
	}
</style>
