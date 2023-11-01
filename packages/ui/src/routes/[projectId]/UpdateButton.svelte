<script lang="ts">
	import type { Loadable } from '@square/svelte-store';
	import { installUpdate, onUpdaterEvent } from '@tauri-apps/api/updater';
	import * as toasts from '$lib/utils/toasts';
	import { onMount } from 'svelte';
	import { relaunch } from '@tauri-apps/api/process';
	import type { Update } from '../updater';

	export let update: Loadable<Update>;

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

{#if $update?.enabled && $update?.shouldUpdate}
	<div class="flex items-center justify-center gap-1">
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
