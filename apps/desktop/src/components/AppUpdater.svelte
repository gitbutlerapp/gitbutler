<script lang="ts">
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import { UPDATER_SERVICE, type InstallStatus } from '$lib/updater/updater';
	import { inject } from '@gitbutler/core/context';
	import { Button, Modal, Markdown } from '@gitbutler/ui';
	import { fade } from 'svelte/transition';
	import { env } from '$env/dynamic/public';

	interface Release {
		version: string;
		notes: string | null;
		released_at: string;
	}

	const updaterService = inject(UPDATER_SERVICE);
	const update = updaterService.update;
	const loading = updaterService.loading;

	let version = $state<string | undefined>();
	let releaseNotes = $state<string | undefined>();
	let status = $state<InstallStatus | undefined>();

	let releaseNotesModal = $state<Modal>();
	let releases = $state<Release[]>([]);
	let currentReleaseIndex = $state(0);
	let loadingReleases = $state(false);

	$effect(() => {
		({ version, releaseNotes, status } = $update);
	});

	async function fetchReleases() {
		if (releases.length > 0) return; // Already fetched

		loadingReleases = true;
		try {
			const response = await fetch(
				'https://app.gitbutler.com/api/downloads?limit=10&channel=release'
			);
			const data = await response.json();
			releases = data.map((r: any) => ({
				version: r.version,
				notes: r.notes,
				released_at: r.released_at
			}));
			// Set current release to the one from the updater if it matches
			if (version) {
				const index = releases.findIndex((r) => r.version === version);
				if (index !== -1) {
					currentReleaseIndex = index;
				}
			}
		} catch (error) {
			console.error('Failed to fetch releases:', error);
		} finally {
			loadingReleases = false;
		}
	}

	function handleOpenModal() {
		fetchReleases();
		releaseNotesModal?.show();
	}

	function goToPreviousRelease() {
		if (currentReleaseIndex > 0) {
			currentReleaseIndex--;
		}
	}

	function goToNextRelease() {
		if (currentReleaseIndex < releases.length - 1) {
			currentReleaseIndex++;
		}
	}

	const currentRelease = $derived(releases.length > 0 ? releases[currentReleaseIndex] : null);

	const displayVersion = $derived(currentRelease ? currentRelease.version : version);

	const displayNotes = $derived(currentRelease ? currentRelease.notes : releaseNotes);

	function handleDismiss() {
		updaterService.dismiss();
	}

	const inFlatpak = $derived(!!env.PUBLIC_FLATPAK_ID);
</script>

{#snippet previousVersionSnippet()}
	{releases[currentReleaseIndex + 1]?.version}
{/snippet}

{#if version || status === 'Up-to-date'}
	<div class="update-banner" data-testid="update-banner" class:busy={$loading}>
		<div class="floating-button">
			<Button icon="cross-small" kind="ghost" onclick={handleDismiss} />
		</div>
		<div class="update-img">
			<div class="update-img__screen">
				{#if status !== 'Done' && status !== 'Up-to-date'}
					<svg
						width="1rem"
						height="1rem"
						viewBox="0 0 18 16"
						fill="none"
						xmlns="http://www.w3.org/2000/svg"
						class="arrow-img"
					>
						<path
							d="M0.647705 7.2L8.64771 14M8.64771 14L16.6477 7.2M8.64771 14V0"
							stroke="var(--clr-scale-pop-50)"
							stroke-width="2"
						/>
					</svg>
				{:else}
					<svg
						width="1.188rem"
						height="0.75rem"
						viewBox="0 0 19 12"
						fill="none"
						xmlns="http://www.w3.org/2000/svg"
					>
						<path
							d="M0.688721 4.06198L7.48872 10.5211L17.6887 0.721069"
							stroke="var(--clr-scale-pop-50)"
							stroke-width="2"
						/>
					</svg>
				{/if}
			</div>

			<svg
				xmlns="http://www.w3.org/2000/svg"
				width="4.375rem"
				height="3.438rem"
				viewBox="0 0 70 55"
				fill="none"
			>
				<path
					opacity="0.9"
					d="M3.68052 9.9494C4.23548 4.4545 9.13986 0.449887 14.6348 1.00485L41.2092 3.68877C46.704 4.24373 50.7086 9.14811 50.1537 14.643L49.7111 19.025C49.3935 22.17 50.5815 25.28 52.9151 27.4122L57.0014 31.146C59.3777 33.3172 60.5635 36.4995 60.1876 39.6963L59.8519 42.5504C59.2143 47.9719 54.3468 51.8803 48.9155 51.3318L9.94939 47.3963C4.45449 46.8414 0.44989 41.937 1.00485 36.4421L3.68052 9.9494Z"
					fill="var(--clr-scale-pop-70)"
				/>
				<path
					opacity="0.6"
					d="M56 21L69 20.2778M56 13.0556L66 8"
					stroke="var(--clr-theme-pop-element)"
					stroke-width="1.5"
				/>
				<path
					d="M14 53.0894L14.3084 39L23.8379 44.4513L33.5791 39.4218L33.2707 53.5112L23.7106 48.0806L14 53.0894Z"
					stroke="var(--clr-scale-pop-50)"
					stroke-width="1.2"
				/>
			</svg>
		</div>

		<h4 class="text-13 text-semibold">
			{#if status === 'Up-to-date'}
				You are up-to-date!
			{:else if status === 'Downloading'}
				Downloading update…
			{:else if status === 'Downloaded'}
				Update downloaded
			{:else if status === 'Installing'}
				Installing update…
			{:else if status === 'Done'}
				Install complete
			{:else if status === 'Checking'}
				Checking for update…
			{:else if status === 'Error'}
				Error occurred
			{:else if version}
				New version available
			{/if}
		</h4>

		<div class="buttons">
			{#if releaseNotes}
				<Button kind="outline" onclick={handleOpenModal}>Release notes</Button>
			{/if}
			{#if !inFlatpak}
				<div class="status-section">
					{#if status !== 'Error' && status !== 'Up-to-date'}
						<div class="sliding-gradient"></div>
					{/if}
					<div class="cta-btn" transition:fade={{ duration: 100 }}>
						{#if !status}
							<Button
								wide
								style="pop"
								testId="download-update"
								onmousedown={async () => {
									await updaterService.downloadAndInstall();
								}}
							>
								Update to {version}
							</Button>
						{:else if status === 'Up-to-date'}
							<Button
								wide
								style="pop"
								testId="got-it"
								onmousedown={async () => {
									updaterService.dismiss();
								}}
							>
								Got it!
							</Button>
						{:else if status === 'Done'}
							<Button
								style="pop"
								wide
								testId="restart-app"
								onclick={async () => await updaterService.relaunchApp()}
							>
								Restart
							</Button>
						{/if}
					</div>
				</div>
			{/if}
		</div>
	</div>
{/if}

{#if releaseNotes}
	<Modal bind:this={releaseNotesModal} width={480} noPadding>
		<ConfigurableScrollableContainer>
			<div class="p-16">
				{#if loadingReleases}
					<div class="loading-state">
						<p class="text-12">Loading releases...</p>
					</div>
				{:else}
					<div class="release-notes-header">
						<h3 class="text-15 text-bold">
							<span class="text-12 m-r-4">📒</span> Release Notes - {displayVersion}
						</h3>

						<div class="flex gap-2">
							<Button
								kind="outline"
								size="tag"
								disabled={currentReleaseIndex === 0}
								onclick={goToPreviousRelease}
								icon="chevron-left"
								reversedDirection
							/>
							<Button
								kind="outline"
								size="tag"
								disabled={currentReleaseIndex === releases.length - 1}
								onclick={goToNextRelease}
								icon="chevron-right"
								children={releases[currentReleaseIndex + 1]?.version
									? previousVersionSnippet
									: undefined}
							></Button>
						</div>
					</div>

					<div class="text-12 text-body release-notes-content">
						<Markdown content={displayNotes || 'No release notes available'} />
					</div>
				{/if}
			</div>
		</ConfigurableScrollableContainer>
	</Modal>
{/if}

<style lang="postcss">
	.update-banner {
		display: flex;
		z-index: var(--z-blocker);
		position: fixed;
		bottom: 12px;
		left: 12px;
		flex-direction: column;
		align-items: center;
		width: 100%;
		max-width: 220px;
		padding: 16px;
		gap: 16px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1);
		cursor: default;
		user-select: none;
	}

	.buttons {
		display: flex;
		flex-direction: column;
		width: 100%;
		gap: 8px;
	}

	/* STATUS SECTION */
	.status-section {
		display: flex;
		position: relative;
		flex-direction: column;
		align-items: center;

		width: 100%;
		overflow: hidden;
		border-radius: var(--radius-m);
		background-color: var(--clr-theme-pop-element);

		transition:
			transform 0.15s ease-in-out,
			height 0.15s ease-in-out;
	}

	.sliding-gradient {
		z-index: 2;
		position: absolute;
		top: 0;
		left: 0;
		width: 200%;
		height: 100%;

		background: linear-gradient(
			80deg,
			rgba(255, 255, 255, 0) 9%,
			rgba(255, 255, 255, 0.5) 31%,
			rgba(255, 255, 255, 0) 75%
		);
		animation: slide 3s ease-in-out infinite;

		mix-blend-mode: overlay;
		pointer-events: none;

		transition: width 0.2s ease-in-out;
	}

	@keyframes slide {
		0% {
			transform: translateX(-100%);
		}
		100% {
			transform: translateX(100%);
		}
	}

	.cta-btn {
		display: flex;
		position: relative;
		width: 100%;
	}

	.busy {
		& .status-section {
			height: 4px;
		}

		& .sliding-gradient {
			width: 100%;
			background: linear-gradient(
				80deg,
				rgba(255, 255, 255, 0) 9%,
				rgba(255, 255, 255, 0.9) 31%,
				rgba(255, 255, 255, 0) 75%
			);
			animation: slide 1.6s ease-in infinite;
		}

		& .arrow-img {
			transform: rotate(180deg);
			animation: moving-arrow 1s ease-out infinite;
		}
	}

	/* IMAGE */
	.update-img {
		display: flex;
		position: relative;
		margin-top: 2px;
		margin-bottom: -4px;
		margin-left: 6px;
	}

	.update-img__screen {
		display: flex;
		position: absolute;
		top: 7px;
		left: 8px;
		align-items: center;
		justify-content: center;
		width: 33px;
		height: 30px;
		overflow: hidden;
		border-radius: 8px;
		background-color: var(--clr-scale-pop-95);
	}

	.floating-button {
		position: absolute;
		top: 8px;
		right: 8px;
	}

	@keyframes moving-arrow {
		0% {
			transform: translateY(-3px);
		}
		100% {
			transform: translateY(3px);
			opacity: 0.3;
		}
	}

	/* RELEASE NOTES MODAL */
	.release-notes-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: 12px;
	}

	.release-notes-content {
		display: flex;
		flex-direction: column;

		& :global(h1) {
			margin-top: 6px;
			font-size: 15px;
		}

		& :global(h2) {
			margin-top: 6px;
			font-size: 13px;
		}
	}

	.loading-state {
		padding: 32px 0;
		color: var(--clr-text-2);
		text-align: center;
	}
</style>
