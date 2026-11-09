<script lang="ts">
	import wavingSvg from "$lib/assets/illustrations/waving.svg?raw";
	import { BACKEND } from "$lib/backend";
	import { SETTINGS_SERVICE } from "$lib/settings/appSettings";
	import { hasNativeApp } from "$lib/settings/nativeApp";
	import { inject } from "@gitbutler/core/context";
	import { persisted } from "@gitbutler/shared/persisted";
	import { Button, Modal } from "@gitbutler/ui-svelte";
	import { onMount } from "svelte";

	const backend = inject(BACKEND);
	const settingsService = inject(SETTINGS_SERVICE);
	const seen = persisted(false, "nativeAppModalSeen");

	let modal = $state<Modal>();
	let switched = $state(false);

	// Shown once, where the native app is built in but not turned on.
	onMount(async () => {
		if ($seen || !(await hasNativeApp(backend))) return;
		const { featureFlags } = await settingsService.fetchAppSettings();
		if (featureFlags.gpui) return;
		modal?.show();
		seen.set(true);
	});

	async function switchToNativeApp() {
		await settingsService.updateFeatureFlags({ gpui: true });
		switched = true;
	}
</script>

<Modal bind:this={modal} width={380} noPadding>
	<div class="native-app__illustration">
		{@html wavingSvg}
	</div>
	<div class="native-app__content">
		{#if switched}
			<h2 class="text-16 text-bold">Restart to open the native app</h2>
			<p class="text-13 text-body">
				The native app opens the next time GitButler starts. To switch back, turn off "Open this app
				at start" in the native app's Settings › Experimental.
			</p>
		{:else}
			<h2 class="text-16 text-bold">Try the new native GitButler app</h2>
			<p class="text-13 text-body">
				It's much faster and takes a fresh approach to the interface. Your projects and settings
				carry over. We're still building it, and it will become the default in a future release.
			</p>
			<p class="text-12 text-body native-app__hint">
				Nothing changes until you restart. Turn it on or off any time in Settings › Experimental.
			</p>
		{/if}
	</div>
	{#snippet controls(close)}
		{#if switched}
			<Button kind="outline" onclick={close}>Later</Button>
			<Button style="pop" onclick={() => backend.relaunch()}>Restart now</Button>
		{:else}
			<Button kind="outline" onclick={close}>Not now</Button>
			<Button style="pop" onclick={switchToNativeApp}>Try the native app</Button>
		{/if}
	{/snippet}
</Modal>

<style lang="postcss">
	/* The illustration comes from but-ui-react, and paints with its tokens. */
	.native-app__illustration {
		--but-background: var(--bg-1);
		--but-muted: var(--bg-2);
		display: flex;
		justify-content: center;
		padding-top: 24px;
		color: var(--border-1);
	}

	.native-app__content {
		display: flex;
		flex-direction: column;
		padding: 16px 16px 20px;
		gap: 6px;
	}

	.native-app__hint {
		margin-top: 6px;
		color: var(--text-2);
	}
</style>
