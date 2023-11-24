<script lang="ts">
	import type { User } from '$lib/backend/cloud';
	import { isLoading, loadStack } from '$lib/backend/ipc';
	import Link from '$lib/components/Link.svelte';
	import TimeAgo from '$lib/components/TimeAgo.svelte';
	import Tooltip from '$lib/components/Tooltip.svelte';
	import Icon from '$lib/icons/Icon.svelte';
	import IconSpinner from '$lib/icons/IconSpinner.svelte';
	import * as events from '$lib/utils/events';
	import { formatDistanceToNowStrict } from 'date-fns';

	export let user: User | undefined;
	export let projectId: string | undefined;
</script>

<div
	class="text-color-3 flex flex-shrink-0 items-center justify-between border-t px-4 py-4"
	style:background-color="var(--bg-surface-highlight)"
	style:border-color="var(--border-surface)"
>
	<div class="flex items-center gap-x-1">
		<Link href="/" class="p-1"><Icon name="home-16" /></Link>
		<Tooltip label="Send feedback">
			<button class="p-1 align-middle" on:click={() => events.emit('openSendIssueModal')}>
				<Icon name="mail-16"></Icon>
			</button>
		</Tooltip>
		<Link href={`/${projectId}/settings`} class="p-1">
			<Icon name="settings-16" />
		</Link>
		{#if $isLoading}
			<Tooltip>
				<IconSpinner class="scale-75" />
				<div slot="label">
					{#each loadStack as item}
						<p>
							{item.name}
							- <TimeAgo date={item.startedAt} addSuffix={true} />
						</p>
					{/each}
				</div>
			</Tooltip>
		{/if}
	</div>
	<Link href="/user/">
		{#if user?.picture}
			<img class="mr-1 inline-block h-5 w-5 rounded-full" src={user.picture} alt="Avatar" />
		{/if}
		{user?.name ?? 'Account'}
	</Link>
</div>
