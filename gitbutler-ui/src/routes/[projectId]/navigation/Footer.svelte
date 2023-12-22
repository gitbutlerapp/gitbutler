<script lang="ts">
	import type { User } from '$lib/backend/cloud';
	import { isLoading, loadStack } from '$lib/backend/ipc';
	import IconButton from '$lib/components/IconButton.svelte';
	import Link from '$lib/components/Link.svelte';
	import TimeAgo from '$lib/components/TimeAgo.svelte';
	import Tooltip from '$lib/components/Tooltip.svelte';
	import Icon from '$lib/icons/Icon.svelte';
	import * as events from '$lib/utils/events';
	import AccountLink from '$lib/components/AccountLink.svelte';

	export let user: User | undefined;
	export let projectId: string | undefined;
</script>

<div class="footer" style:border-color="var(--clr-theme-container-outline-light)">
	<div class="left-btns">
		<Link href="/"><IconButton icon="home" /></Link>
		<Tooltip label="Send feedback">
			<IconButton icon="mail" on:click={() => events.emit('openSendIssueModal')}></IconButton>
		</Tooltip>
		<Link href={`/${projectId}/settings`}>
			<IconButton icon="settings" />
		</Link>
		{#if $isLoading}
			<div class="loading-status">
				<Tooltip>
					<Icon name="spinner" />
					<div slot="label">
						{#each loadStack as item}
							<p>
								{item.name}
								- <TimeAgo date={item.startedAt} addSuffix={true} />
							</p>
						{/each}
					</div>
				</Tooltip>
			</div>
		{/if}
	</div>
	<AccountLink {user} />
</div>

<style lang="postcss">
	.footer {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: var(--space-12);
		border-top: 1px solid var(--clr-theme-container-outline-light);
	}

	.left-btns {
		display: flex;
		align-items: center;
	}
	.loading-status {
		margin-left: 0.5rem;
		margin-right: 0.5rem;
	}
</style>
