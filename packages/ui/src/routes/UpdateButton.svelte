<script lang="ts">
	import * as toasts from '$lib/toasts';
	import { Button } from '$lib/components';
	import type { Update } from '$lib/updater';
	import { installUpdate } from '@tauri-apps/api/updater';
	import { getVersion } from '@tauri-apps/api/app';

	export let update: Update;

	let installing = false;
	const onUpdateClicked = async () => {
		installing = true;
		await installUpdate()
			.finally(() => {
				installing = false;
			})
			.catch((e) => {
				toasts.error(e.message);
			});
	};
</script>

{#if update?.enabled}
	{#if update.shouldUpdate}
		<Button
			loading={installing}
			color="purple"
			kind="filled"
			height="small"
			on:click={onUpdateClicked}>Update to {update.version}</Button
		>
	{:else}
		<Button color="purple" kind="filled" height="small" disabled>Up to date</Button>
	{/if}
{/if}
