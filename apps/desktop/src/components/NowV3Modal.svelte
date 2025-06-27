<script lang="ts">
	import tryV3Svg from '$lib/assets/try-v3.svg?raw';
	import { SettingsService } from '$lib/config/appSettingsV2';
	import { createKeybind } from '$lib/utils/hotkeys';
	import { getContext } from '@gitbutler/shared/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import Link from '@gitbutler/ui/link/Link.svelte';
	import { onMount } from 'svelte';

	let modalRef = $state<ReturnType<typeof Modal>>();
	const settingsService = getContext(SettingsService);
	const settingsStore = settingsService.appSettings;
	const isV3Enabled = $derived($settingsStore?.featureFlags.v3);

	// Used to track whether we have performed the automatic switch to V3.
	// Once set, we won't try to switch to V3 again automatically.
	const switchedToV3 = persisted<boolean>(false, 'switchedToV3');
	// Used to track whether we should display the modal. We use a persisted
	// because we do a `location.reload()` which means we need to talk between
	// the onMount logic and the modal after it is re-rendered.
	const showModal = persisted<boolean>(false, 'needToDisplayModal');

	onMount(async () => {
		if (isV3Enabled) {
			// If the user switches back to v2, then we don't want to automatically switch to v3 again.
			$switchedToV3 = true;
			return;
		}

		if (!$switchedToV3) {
			await settingsService.updateFeatureFlags({ v3: true });
			location.reload();
			$showModal = true;
			$switchedToV3 = true;
		}
	});

	$effect(() => {
		if ($showModal) {
			modalRef?.show();
		}
	});

	const handleKeyDown = createKeybind({
		// Toggle v3 design on/off
		'd e b u g s w i t c h': async () => {
			$switchedToV3 = false;

			await settingsService.updateFeatureFlags({ v3: false });
			location.reload();
		}
	});
</script>

<svelte:window onkeydown={handleKeyDown} />

<Modal
	width={434}
	bind:this={modalRef}
	noPadding
	onSubmit={async (close) => {
		close();
	}}
	onClose={() => {
		$showModal = false;
	}}
>
	<div class="modal-wrapper">
		<div class="modal-illustration-wrapper">
			<div class="modal-illustration__svg">
				{@html tryV3Svg}
			</div>
		</div>
		<div class="modal-content">
			<div class="modal-description">
				<h2 class="text-16 text-bold">Welcome to the new GitButler UI!</h2>
				<p class="text-13 text-body">
					We’ve refreshed the interface — and we’d love to hear what you think.
				</p>
				<p class="text-13 text-body">
					Join the conversation on <Link href="https://discord.gg/MmFkmaJ42D">Discord</Link>, or
					open an issue on <Link
						href="https://github.com/gitbutlerapp/gitbutler/issues/new?template=BLANK_ISSUE"
						>GitHub</Link
					>.
				</p>
			</div>

			<div class="modal-content__notes text-12 text-body">
				<p>
					If needed, you can temporarily switch back to the old UI under Experimental in Global
					Settings, but heads up — this option will be removed in a future release.
				</p>

				<p class="clr-text-2">
					A restart may be needed when switching back to the old UI for the change to fully take
					effect
				</p>
			</div>
		</div>
	</div>
	{#snippet controls()}
		<div class="modal-footer">
			<Button style="pop" type="submit">Ok</Button>
		</div>
	{/snippet}
</Modal>

<style lang="postcss">
	.modal-wrapper {
		display: flex;
		flex-direction: column;
	}

	.modal-illustration-wrapper {
		position: relative;
		height: 176px;
		background-color: var(--clr-illustration-bg);
	}

	.modal-illustration__svg {
		position: absolute;
		bottom: 0;
		left: 16px;
		width: 404px;
		height: 158px;
	}

	.modal-content {
		display: flex;
		flex-direction: column;
		padding: 20px 16px 16px;
		gap: 10px;
	}

	.modal-description {
		display: flex;
		flex-direction: column;
		max-width: 380px;
		gap: 10px;
	}

	.modal-content__notes {
		display: flex;
		flex-direction: column;
		margin-top: 8px;
		padding: 12px;
		gap: 2px;
		/* background-color: var(--clr-bg-1-muted); */
		border: 1px solid var(--clr-border-3);
		border-radius: var(--radius-m);
	}

	.modal-footer {
		display: flex;
		width: 100%;
		gap: 6px;
	}

	.modal-footer__checkbox {
		display: flex;
		flex: 1;
		align-items: center;
		gap: 8px;
	}
</style>
