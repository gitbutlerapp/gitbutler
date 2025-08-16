<script lang="ts">
	import { showToast } from '$lib/notifications/toasts';
	import { UPDATER_SERVICE, type InstallStatus } from '$lib/updater/updater';
	import { inject } from '@gitbutler/shared/context';
	import { Button } from '@gitbutler/ui';
	import { fade } from 'svelte/transition';
	import { env } from '$env/dynamic/public';

	const updaterService = inject(UPDATER_SERVICE);
	const update = updaterService.update;
	const loading = updaterService.loading;

	let version = $state<string | undefined>();
	let releaseNotes = $state<string | undefined>();
	let status = $state<InstallStatus | undefined>();

	$effect(() => {
		({ version, releaseNotes, status } = $update);
	});

	function handleDismiss() {
		updaterService.dismiss();
	}

	const inFlatpak = $derived(!!env.PUBLIC_FLATPAK_ID);
</script>

{#if version || status === 'Up-to-date'}
	<div class="update-banner" data-testid="update-banner" class:busy={$loading}>
		<div class="floating-button">
			<Button icon="cross-small" kind="ghost" onclick={handleDismiss} />
		</div>
		<div class="img">
			<div class="circle-img">
				{#if status !== 'Done' && status !== 'Up-to-date'}
					<svg
						class="arrow-img"
						width="12"
						height="34"
						viewBox="0 0 12 34"
						fill="none"
						xmlns="http://www.w3.org/2000/svg"
					>
						<path
							d="M6 21V32.5M6 32.5L1 27.5M6 32.5L11 27.5"
							stroke="var(--clr-scale-ntrl-100)"
							stroke-width="1.5"
						/>
						<path
							d="M6 0V11.5M6 11.5L1 6.5M6 11.5L11 6.5"
							stroke="var(--clr-scale-ntrl-100)"
							stroke-width="1.5"
						/>
					</svg>
				{:else}
					<svg
						class="tick-img"
						width="14"
						height="11"
						viewBox="0 0 14 11"
						fill="none"
						xmlns="http://www.w3.org/2000/svg"
					>
						<path
							d="M1 4.07692L5.57143 9L13 1"
							stroke="var(--clr-scale-ntrl-100)"
							stroke-width="1.5"
						/>
					</svg>
				{/if}
			</div>

			<svg
				width="60"
				height="36"
				viewBox="0 0 60 36"
				fill="none"
				xmlns="http://www.w3.org/2000/svg"
			>
				<path
					d="M31.5605 35.5069C31.4488 35.5097 31.3368 35.5112 31.2245 35.5112H12.8571C5.75633 35.5112 0 29.7548 0 22.654C0 15.5532 5.75634 9.79688 12.8571 9.79688H16.123C18.7012 4.02354 24.493 0 31.2245 0C39.7331 0 46.7402 6.42839 47.6541 14.6934H49.5918C55.3401 14.6934 60 19.3533 60 25.1015C60 30.8498 55.3401 35.5097 49.5918 35.5097H32.4489C32.2692 35.5097 32.0906 35.5051 31.913 35.4961C31.7958 35.5009 31.6783 35.5045 31.5605 35.5069Z"
					fill="var(--clr-scale-pop-70)"
				/>
				<g opacity="0.4">
					<path
						d="M39 35.5102V29.2505H29.25V9.75049H39V19.5005H48.75V29.2505H58.5V30.4877C56.676 33.4983 53.3688 35.5102 49.5918 35.5102H39Z"
						fill="var(--clr-scale-pop-50)"
					/>
					<path
						d="M46.3049 9.75049H39V1.93967C42.2175 3.65783 44.8002 6.4091 46.3049 9.75049Z"
						fill="var(--clr-scale-pop-50)"
					/>
					<path
						d="M9.75 35.1337C10.745 35.3806 11.7858 35.5117 12.8571 35.5117H29.25V29.2505H9.75V19.5005H19.5V9.75049H29.25V0.117188C25.4568 0.568673 22.0577 2.30464 19.5 4.87786V9.75049H16.144C16.137 9.7661 16.13 9.78173 16.123 9.79737H12.8571C11.7858 9.79737 10.745 9.92841 9.75 10.1753V19.5005H0.389701C0.135193 20.5097 0 21.5663 0 22.6545C0 25.0658 0.663785 27.322 1.81859 29.2505H9.75V35.1337Z"
						fill="var(--clr-scale-pop-50)"
					/>
				</g>
			</svg>
		</div>

		<h4 class="text-13 label">
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
				<Button
					kind="outline"
					onmousedown={() => {
						showToast({
							id: 'release-notes',
							title: `Release notes for ${version}`,
							message: releaseNotes || 'no release notes available'
						});
					}}
				>
					Release notes
				</Button>
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
		padding: 24px;
		gap: 16px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1);
		cursor: default;
		user-select: none;
	}

	.label {
		color: var(--clr-scale-ntrl-0);
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
			animation: moving-arrow 1s ease-in-out infinite;
		}
	}

	/* IMAGE */

	.img {
		position: relative;
		margin-bottom: 4px;
	}

	.circle-img {
		position: absolute;
		bottom: -8px;
		left: 17px;
		width: 26px;
		height: 26px;
		overflow: hidden;
		border-radius: 50%;
		background-color: var(--clr-scale-pop-40);
		transition: transform 0.2s ease-in-out;

		&:after {
			position: absolute;
			top: 0;
			left: 0;
			width: 100%;
			height: 100%;
			border-radius: 50%;
			background-color: transparent;
			box-shadow: inset 0 0 4px 4px var(--clr-scale-pop-40);
			content: '';
		}
	}

	.arrow-img {
		position: absolute;
		top: -14px;
		left: 7px;
	}

	.tick-img {
		position: absolute;
		top: 8px;
		left: 6px;
	}

	.floating-button {
		position: absolute;
		top: 10px;
		right: 10px;
	}

	@keyframes moving-arrow {
		0% {
			transform: translateY(0);
		}
		100% {
			transform: translateY(21px);
		}
	}
</style>
