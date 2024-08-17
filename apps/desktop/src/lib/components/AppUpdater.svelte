<script lang="ts">
	import Link from '../shared/Link.svelte';
	import newVersionSvg from '$lib/assets/empty-state/app-new-version.svg?raw';
	import upToDateSvg from '$lib/assets/empty-state/app-up-to-date.svg?raw';
	import { UpdaterService, type Update } from '$lib/backend/updater';
	import { getContext } from '$lib/utils/context';
	import Button from '@gitbutler/ui/inputs/Button.svelte';
	import Modal from '@gitbutler/ui/modal/Modal.svelte';

	const updaterService = getContext(UpdaterService);
	const currentVersion = updaterService.currentVersion;

	const update = updaterService.update;

	let status = $state<Update['status']>();
	let version = $state<Update['version']>();
	let lastVersion: string | undefined;
	let dismissed = false;
	let modal: Modal;

	$effect(() => {
		if ($update) {
			console.log($update);
			handleUpdate($update);
		}
	});

	function handleUpdate(update: Update) {
		version = update?.version;

		if (version !== lastVersion) {
			dismissed = false;
		}

		status = update?.status;
		const manual = update?.manual;

		if (manual) {
			modal.show();
		} else if (status === 'ERROR') {
			modal.close();
		} else if (status && status !== 'UPTODATE' && !dismissed) {
			modal.show();
		} else if (version && !dismissed) {
			modal.show();
		}
		lastVersion = version;
	}

	function handleDismiss() {
		dismissed = true;
		modal.close();
	}
</script>

<Modal width="xsmall" bind:this={modal}>
	<div class="modal-illustration">
		{#if status === 'UPTODATE' || status === 'DONE'}
			{@html upToDateSvg}
		{:else}
			{@html newVersionSvg}
		{/if}
	</div>

	<h3 class="text-serif-32 modal-title">
		{#if status === 'UPTODATE'}
			You are up-to-date
		{:else if status === 'DONE'}
			Install complete!
		{:else}
			New {version} version
		{/if}
	</h3>

	<p class="text-13 text-body modal-caption">
		{#if status === 'UPTODATE'}
			You're on GitButler {$currentVersion}, which is the most up-to-date version.
		{:else}
			Upgrade now for the latest features.
			<br />
			You can find release notes <Link href="https://discord.gg/WnTgfnmS">here</Link>.
		{/if}
	</p>

	<div class="modal-actions">
		{#if status !== 'UPTODATE' && status !== 'DONE'}
			<Button
				style="ghost"
				outline
				disabled={status === 'PENDING' || status === 'DOWNLOADED'}
				onclick={handleDismiss}
			>
				Cancel
			</Button>
		{/if}

		{#if status === 'UPTODATE'}
			<Button style="pop" kind="solid" wide outline onclick={handleDismiss}>Got it!</Button>
		{:else if status === 'DONE'}
			<Button
				style="pop"
				kind="solid"
				wide
				outline
				onclick={() => {
					updaterService.relaunchApp();
				}}
			>
				Restart
			</Button>
		{:else}
			<Button
				style="pop"
				kind="solid"
				wide
				loading={status === 'PENDING' || status === 'DOWNLOADED'}
				onclick={() => {
					updaterService.installUpdate();
				}}
			>
				{#if status === 'PENDING'}
					Downloading update...
				{:else if status === 'DOWNLOADED'}
					Installing update...
				{:else}
					Download {version}
				{/if}
			</Button>
		{/if}
	</div>
</Modal>

<style lang="postcss">
	.modal-illustration {
		display: flex;
		margin-bottom: 16px;
	}

	.modal-title {
		word-wrap: break-word;
		margin-bottom: 10px;
	}

	.modal-caption {
		color: var(--clr-text-2);
		margin-bottom: 24px;
	}

	.modal-actions {
		display: flex;
		gap: 8px;
	}
</style>
