<script lang="ts">
	import type { User } from '$lib/backend/cloud';
	import { isLoading, loadStack } from '$lib/backend/ipc';
	import IconButton from '$lib/components/IconButton.svelte';
	import Link from '$lib/components/Link.svelte';
	import TimeAgo from '$lib/components/TimeAgo.svelte';
	import Tooltip from '$lib/components/Tooltip.svelte';
	import IconSpinner from '$lib/icons/IconSpinner.svelte';
	import * as events from '$lib/utils/events';
	import AccountLink from './AccountLink.svelte';

	export let user: User | undefined;
	export let projectId: string | undefined;
</script>

<div
	class="text-color-3 flex flex-shrink-0 items-center justify-between border-t px-3 py-3"
	style:border-color="var(--clr-theme-container-outline-light)"
>
	<div class="flex items-center">
		<Link href="/" class="p-1"><IconButton icon="home" /></Link>
		<Tooltip label="Send feedback">
			<IconButton icon="mail" on:click={() => events.emit('openSendIssueModal')}></IconButton>
		</Tooltip>
		<Link href={`/${projectId}/settings`} class="p-1">
			<IconButton icon="settings" />
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
	<AccountLink {user} />
</div>
