<script lang="ts">
	import { fade } from 'svelte/transition';

	import { installUpdate, onUpdaterEvent } from '@tauri-apps/api/updater';
	import * as toasts from '$lib/utils/toasts';
	import { onMount } from 'svelte';
	import { relaunch } from '@tauri-apps/api/process';
	import type { Update } from '../../routes/updater';
	import type { Observable } from 'rxjs';

	import Button from './Button.svelte';

	export let update: Observable<Update>;

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

	// fake update
	// variables
	let isDownloading = false;
	let isInstalling = false;
	let isDone = false;

	// functions
	const downloadUpdate = () => {
		isDownloading = true;
		setTimeout(() => {
			isDownloading = false;
			isInstalling = true;
		}, 3000);

		setTimeout(() => {
			isInstalling = false;
			isDone = true;
		}, 6000);
	};
</script>

<div class="update-banner" class:busy={isDownloading || isInstalling}>
	<div class="img">
		<div class="circle-img">
			{#if !isDone}
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
						stroke="var(--clr-theme-scale-ntrl-100)"
						stroke-width="1.5"
					/>
					<path
						d="M6 0V11.5M6 11.5L1 6.5M6 11.5L11 6.5"
						stroke="var(--clr-theme-scale-ntrl-100)"
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
					<path d="M1 4.07692L5.57143 9L13 1" stroke="white" stroke-width="1.5" />
				</svg>
			{/if}
		</div>

		<svg width="60" height="36" viewBox="0 0 60 36" fill="none" xmlns="http://www.w3.org/2000/svg">
			<path
				d="M31.5605 35.5069C31.4488 35.5097 31.3368 35.5112 31.2245 35.5112H12.8571C5.75633 35.5112 0 29.7548 0 22.654C0 15.5532 5.75634 9.79688 12.8571 9.79688H16.123C18.7012 4.02354 24.493 0 31.2245 0C39.7331 0 46.7402 6.42839 47.6541 14.6934H49.5918C55.3401 14.6934 60 19.3533 60 25.1015C60 30.8498 55.3401 35.5097 49.5918 35.5097H32.4489C32.2692 35.5097 32.0906 35.5051 31.913 35.4961C31.7958 35.5009 31.6783 35.5045 31.5605 35.5069Z"
				fill="var(--clr-theme-scale-pop-70)"
			/>
			<g opacity="0.4">
				<path
					d="M39 35.5102V29.2505H29.25V9.75049H39V19.5005H48.75V29.2505H58.5V30.4877C56.676 33.4983 53.3688 35.5102 49.5918 35.5102H39Z"
					fill="var(--clr-theme-scale-pop-50)"
				/>
				<path
					d="M46.3049 9.75049H39V1.93967C42.2175 3.65783 44.8002 6.4091 46.3049 9.75049Z"
					fill="var(--clr-theme-scale-pop-50)"
				/>
				<path
					d="M9.75 35.1337C10.745 35.3806 11.7858 35.5117 12.8571 35.5117H29.25V29.2505H9.75V19.5005H19.5V9.75049H29.25V0.117188C25.4568 0.568673 22.0577 2.30464 19.5 4.87786V9.75049H16.144C16.137 9.7661 16.13 9.78173 16.123 9.79737H12.8571C11.7858 9.79737 10.745 9.92841 9.75 10.1753V19.5005H0.389701C0.135193 20.5097 0 21.5663 0 22.6545C0 25.0658 0.663785 27.322 1.81859 29.2505H9.75V35.1337Z"
					fill="var(--clr-theme-scale-pop-50)"
				/>
			</g>
		</svg>
	</div>

	<h4 class="text-base-13 label">
		{#if isDownloading}
			Downloading update...
		{:else if isInstalling}
			Installing update...
		{:else if isDone}
			Update installed!
		{:else}
			New version available
		{/if}
	</h4>

	<div class="status-section">
		<div class="sliding-gradient" />

		{#if !isDownloading && !isInstalling && !isDone}
			<div class="cta-btn" class:busy={isDownloading} transition:fade={{ duration: 100 }}>
				<Button wide on:click={downloadUpdate}>Download v.0.3.4</Button>
			</div>
		{/if}

		{#if isDone}
			<div class="cta-btn" class:busy={isDownloading} transition:fade={{ duration: 100 }}>
				<Button wide>Restart to update</Button>
			</div>
		{/if}
	</div>
</div>

<!-- {#if $update?.enabled && $update?.shouldUpdate}
	<div class="flex items-center justify-center gap-1">
		asd
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
{/if} -->

<style lang="postcss">
	.update-banner {
		cursor: default;
		user-select: none;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-16);

		width: 100%;
		max-width: 220px;

		position: fixed;
		bottom: var(--space-12);
		left: var(--space-12);
		padding: var(--space-24);
		background-color: var(--clr-theme-container-light);
		border: 1px solid var(--clr-theme-container-outline-light);
		border-radius: var(--radius-m);
	}

	.label {
		color: var(--clr-theme-scale-ntrl-0);
	}

	/* STATUS SECTION */

	.status-section {
		position: relative;
		overflow: hidden;

		display: flex;
		flex-direction: column;
		align-items: center;

		height: var(--size-btn-m);
		min-width: 160px;
		background-color: var(--clr-theme-pop-element);
		border-radius: var(--radius-m);

		transition:
			transform 0.15s ease-in-out,
			height 0.15s ease-in-out;
	}

	.sliding-gradient {
		pointer-events: none;
		z-index: 2;
		position: absolute;
		top: 0;
		left: 0;
		width: 200%;
		height: 100%;

		mix-blend-mode: overlay;

		background: linear-gradient(
			80deg,
			rgba(255, 255, 255, 0) 9%,
			rgba(255, 255, 255, 0.5) 31%,
			rgba(255, 255, 255, 0) 75%
		);
		animation: slide 3s ease-in-out infinite;

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
		width: 100%;
		z-index: 1;
		position: relative;
	}

	.busy {
		& .status-section {
			height: var(--space-4);
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
		overflow: hidden;
		bottom: -8px;
		left: 17px;
		width: 26px;
		height: 26px;
		border-radius: 50%;
		background-color: var(--clr-theme-scale-pop-40);
		transition: transform 0.2s ease-in-out;

		&:after {
			content: '';
			position: absolute;
			top: 0;
			left: 0;
			width: 100%;
			height: 100%;
			background-color: transparent;
			box-shadow: inset 0 0 4px 4px var(--clr-theme-scale-pop-40);
			border-radius: 50%;
		}
	}

	.arrow-img {
		position: absolute;
		top: -14px;
		left: 7px;
		/* transform: translateY(20px); */
	}

	.tick-img {
		position: absolute;
		top: 8px;
		left: 6px;
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
