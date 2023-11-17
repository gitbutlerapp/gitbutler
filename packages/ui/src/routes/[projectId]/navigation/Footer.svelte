<script lang="ts">
	import type { User } from '$lib/backend/cloud';
	import { isLoading, loadStack } from '$lib/backend/ipc';
	import Link from '$lib/components/Link.svelte';
	import Tooltip from '$lib/components/Tooltip.svelte';
	import IconEmail from '$lib/icons/IconEmail.svelte';
	import IconHome from '$lib/icons/IconHome.svelte';
	import IconSettings from '$lib/icons/IconSettings.svelte';
	import IconSpinner from '$lib/icons/IconSpinner.svelte';
	import * as events from '$lib/utils/events';

	export let user: User | undefined;
	export let projectId: string | undefined;
</script>

<div
	class="text-color-3 flex flex-shrink-0 items-center justify-between border-t px-4 py-4"
	style:background-color="var(--bg-surface-highlight)"
	style:border-color="var(--border-surface)"
>
	<div class="flex items-center gap-x-1">
		<Link href="/" class="p-1"><IconHome /></Link>
		<Tooltip label="Send feedback">
			<button class="p-1 align-middle" on:click={() => events.emit('openSendIssueModal')}>
				<IconEmail class="align-top" />
			</button>
		</Tooltip>
		<Link href={`/${projectId}/settings`} class="p-1">
			<IconSettings />
		</Link>
		{#if $isLoading}
			<Tooltip label={loadStack.join('\n')}>
				<IconSpinner class="scale-75" />
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
