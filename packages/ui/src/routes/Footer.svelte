<script lang="ts">
	import type { Project } from '$lib/api/ipc/projects';
	import Link from '$lib/components/Link/Link.svelte';
	import Tooltip from '$lib/components/Tooltip/Tooltip.svelte';
	import IconEmail from '$lib/icons/IconEmail.svelte';
	import type { Loadable } from '@square/svelte-store';
	import { installUpdate, onUpdaterEvent } from '@tauri-apps/api/updater';
	import * as toasts from '$lib/toasts';
	import { onMount } from 'svelte';
	import { relaunch } from '@tauri-apps/api/process';
	import * as events from '$lib/events';

	export let project: Project | undefined;
	export let update: Loadable<any>;

	let updateStatus: {
		error?: string;
		status: 'PENDING' | 'DOWNLOADED' | 'ERROR' | 'DONE' | 'UPTODATE';
	};

	onMount(() => {
		const unsubscribe = onUpdaterEvent((status) => {
			updateStatus = status;
			if (updateStatus.error) {
				toasts.error(updateStatus.error);
			}
		});
		return () => unsubscribe.then((unsubscribe) => unsubscribe());
	});

	let updateTimer: ReturnType<typeof setInterval>;
	onMount(() => {
		update.load();
		const tenMinutes = 1000 * 60 * 10;
		updateTimer = setInterval(() => {
			update.reload?.();
		}, tenMinutes);
		return () => {
			() => clearInterval(updateTimer);
		};
	});
</script>

<footer class="text-color-3 w-full text-sm font-medium">
	<div
		class="bg-color-3 border-color-4 flex h-[1.375rem] flex-shrink-0 select-none items-center border-t"
	>
		<div class="mx-4 flex w-full flex-row items-center justify-between space-x-2 pb-[1px]">
			<div>
				{#if project}
					<Link href="/{project?.id}/settings">
						<div class="flex flex-row items-center space-x-2">
							{#if project?.api?.sync}
								<div class="h-2 w-2 rounded-full bg-green-700" />
								<span>online</span>
							{:else}
								<div class="h-2 w-2 rounded-full bg-red-700" />
								<span class="text-light-600 dark:text-dark-200">offline</span>
							{/if}
						</div>
					</Link>
				{/if}
			</div>

			<div class="flex gap-2">
				{#if $update?.enabled && $update?.shouldUpdate}
					<div class="flex items-center gap-1">
						{#if !updateStatus}
							<div
								class="mr-1 flex h-4 w-4 items-center justify-center rounded-full bg-red-500 text-xs font-bold text-white"
							>
								1
							</div>
							<button on:click={() => installUpdate()}>
								version {$update.version} available
							</button>
						{:else if updateStatus.status === 'PENDING'}
							<span>downloading update...</span>
						{:else if updateStatus.status === 'DOWNLOADED'}
							<span>installing update...</span>
						{:else if updateStatus.status === 'DONE'}
							<button on:click={() => relaunch()}>restart to update</button>
						{/if}
					</div>
				{/if}

				<div class="flex items-center gap-1">
					<Tooltip label="Send feedback">
						<IconEmail class="h-4 w-4" on:click={() => events.emit('openSendIssueModal')} />
					</Tooltip>
				</div>
			</div>
		</div>
	</div>
</footer>
