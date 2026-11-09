<script lang="ts">
	import badgeSvg from "$lib/assets/illustrations/native-app-badge.svg?raw";
	import windowSvg from "$lib/assets/illustrations/native-app-window.svg?raw";
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

<Modal bind:this={modal} width={560} noPadding>
	<div class="native-app">
		<div class="native-app__illustration">
			<div class="native-app__window">{@html windowSvg}</div>
			<div class="native-app__badge">{@html badgeSvg}</div>
		</div>
		<div class="native-app__content">
			{#if switched}
				<h2 class="text-16 text-bold">Restart to open the native app</h2>
				<p class="text-13 text-body">
					The native app opens the next time GitButler starts. To switch back, turn off "Open this
					app at start" in the native app's Settings › Experimental.
				</p>
			{:else}
				<h2 class="text-16 text-bold">Try the new native GitButler app</h2>
				<p class="text-13 text-body">
					It's much faster and takes a fresh approach to the interface. Your projects and settings
					carry over. We're still building it, and it will become the default in a future release.
				</p>
				<p class="text-13 text-body native-app__hint">
					Nothing changes until you restart. Turn it on or off any time in Settings › Experimental.
				</p>
			{/if}
		</div>
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
	.native-app {
		display: flex;
		align-items: flex-start;
		padding: 24px 24px 24px 28px;
		gap: 24px;
	}

	/* The window and its badge sit on the scene's tile, as the welcome screen's art does. */
	.native-app__illustration {
		position: relative;
		flex-shrink: 0;
		width: 159px;
		height: 131px;
		border-radius: var(--radius-l);
		background-color: var(--art-scene-bg);
	}

	.native-app__window {
		display: flex;
		position: absolute;
		top: 23.4px;
		left: 17.4px;
	}

	.native-app__badge {
		--native-app-sticker: var(--art-scene-outline);
		--native-app-sticker-text: var(--art-scene-fill);
		--native-app-sticker-ring: transparent;
		display: flex;
		position: absolute;
		top: 73.91px;
		left: 82.2px;
		transform: rotate(-7.75deg);
	}

	/* A dark sticker would sink into the dark modal, so in dark mode it turns pale, with a ring
	   of the modal's ground to lift it off the tile. */
	:global(.dark) .native-app__badge {
		--native-app-sticker: var(--art-scene-fill);
		--native-app-sticker-text: var(--art-scene-outline);
		--native-app-sticker-ring: var(--bg-1);
	}

	.native-app__content {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.native-app__hint {
		color: var(--text-2);
	}
</style>
