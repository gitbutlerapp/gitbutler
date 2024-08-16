<script lang="ts">
	import Link from '../shared/Link.svelte';
	import newVersionSvg from '$lib/assets/empty-state/app-new-version.svg?raw';
	import upToDateSvg from '$lib/assets/empty-state/app-up-to-date.svg?raw';
	import { UpdaterService } from '$lib/backend/updater';
	import { getContext } from '$lib/utils/context';
	import Button from '@gitbutler/ui/inputs/Button.svelte';
	import Modal from '@gitbutler/ui/modal/Modal.svelte';

	const updaterService = getContext(UpdaterService);
	const update = updaterService.update;
	const currentVersion = updaterService.currentVersion;

	const status = $derived($update?.status);
	const version = $derived($update?.version);

	let dismissed = $state(false);
	let open = $state(false);

	let modalRef: Modal;

	$effect(() => {
		if (version || (status && status !== 'ERROR' && !dismissed && !open)) {
			open = true;
		}
	});

	$effect(() => {
		if (status === 'ERROR') {
			open = false;
		}
	});

	$effect(() => {
		if (open) {
			modalRef.show();
		} else {
			modalRef.close();
		}
	});

	function handleDismiss() {
		dismissed = true;
		open = false;
	}
</script>

<Modal width="xsmall" bind:this={modalRef}>
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
		{:else}
			<Button
				style="pop"
				kind="solid"
				wide
				loading={status === 'PENDING' || status === 'DOWNLOADED'}
				onclick={async () => {
					await updaterService.installUpdate();
				}}
			>
				{#if status === 'PENDING'}
					Downloading update...
				{:else if status === 'DOWNLOADED'}
					Installing update...
				{:else if status === 'DONE'}
					Restart
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
