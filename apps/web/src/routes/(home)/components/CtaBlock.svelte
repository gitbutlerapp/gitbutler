<script lang="ts">
	import * as jsonLinks from '$home/data/links.json';
	import { clickOutside } from '$lib/hooks/clickOutside';
	import { targetDownload } from '$lib/store';
	import { latestClientVersion } from '$lib/store';
	import { quadIn } from 'svelte/easing';
	import { fly } from 'svelte/transition';

	let videoElement = $state<HTMLVideoElement>();
	interface Props {
		secondButton?: {
			label: string;
			url: string;
		} | null;
		lightTheme?: boolean;
		showVideoDemo?: boolean;
	}

	let { secondButton = null, lightTheme = false, showVideoDemo = true }: Props = $props();

	let showSelect = $state(false);
	let selectElement = $state<HTMLElement>();

	function handleShowSelect(e: MouseEvent | KeyboardEvent) {
		e.stopPropagation();
		showSelect = !showSelect;
	}

	function handleChangeDownloadLink(e: MouseEvent) {
		const target = e.target as HTMLButtonElement;
		const targetOsId = target.getAttribute('data-targetOsId');
		if (targetOsId) {
			const entries = Object.entries(jsonLinks.downloads);
			const newTarget = entries.find(([k, _v]) => k === targetOsId);
			if (newTarget) {
				targetDownload.set(newTarget[1]);
			}
		}

		if (!targetOsId) return;

		showSelect = false;
	}

	function handleClickOutside() {
		showSelect = false;
	}
</script>

<section class="wrapper">
	<div class="download-wrapper">
		{#if showSelect}
			<section
				class="os-select"
				role="radiogroup"
				tabindex="0"
				bind:this={selectElement}
				transition:fly={{ duration: 50, easing: quadIn, y: 10, x: 0 }}
				onintrostart={() => {
					selectElement?.focus();
				}}
				use:clickOutside={{ handler: handleClickOutside }}
			>
				<div class="os-select__section">
					<img
						class="os-select__section-os-icon"
						src="/images/os-icons/apple-small-logo.svg"
						alt=""
					/>
					<div class="os-select__subsection">
						<button
							type="button"
							class="os-select__item"
							data-targetOsId={jsonLinks.downloads.appleSilicon.id}
							onclick={handleChangeDownloadLink}
						>
							{jsonLinks.downloads.appleSilicon.label}
						</button>
						<button
							type="button"
							class="os-select__item"
							data-targetOsId={jsonLinks.downloads.intelMac.id}
							onclick={handleChangeDownloadLink}
						>
							{jsonLinks.downloads.intelMac.label}
						</button>

						<div class="os-select__divider"></div>
					</div>
				</div>
				<div class="os-select__section">
					<img
						class="os-select__section-os-icon"
						src="/images/os-icons/windows-small-logo.svg"
						alt=""
					/>
					<div class="os-select__subsection">
						<button
							type="button"
							class="os-select__item"
							data-targetOsId={jsonLinks.downloads.windowsMsi.id}
							onclick={handleChangeDownloadLink}
						>
							{jsonLinks.downloads.windowsMsi.label}
						</button>
						<div class="os-select__divider"></div>
					</div>
				</div>
				<div class="os-select__section">
					<img
						class="os-select__section-os-icon"
						src="/images/os-icons/linux-small-logo.svg"
						alt=""
					/>
					<div class="os-select__subsection">
						<button
							type="button"
							class="os-select__item"
							data-targetOsId={jsonLinks.downloads.linuxDeb.id}
							onclick={handleChangeDownloadLink}
						>
							{jsonLinks.downloads.linuxDeb.label}
						</button>
						<button
							type="button"
							class="os-select__item"
							data-targetOsId={jsonLinks.downloads.linuxAppimage.id}
							onclick={handleChangeDownloadLink}
						>
							{jsonLinks.downloads.linuxAppimage.label}
						</button>
					</div>
				</div>
			</section>
		{/if}

		<div class="download-wrapper-head">
			<a
				class="download-btn"
				class:btn-blue={!lightTheme}
				class:btn-white={lightTheme}
				href={$targetDownload.url}
			>
				<span>Download</span>
			</a>

			<div
				class="select-btn"
				role="button"
				class:btn-blue={!lightTheme}
				class:btn-white={lightTheme}
				tabindex="0"
				onclick={handleShowSelect}
				onkeydown={handleShowSelect}
			>
				<div class="divider"></div>
				<span>{$targetDownload.label}</span>
				<svg
					width="20"
					height="20"
					viewBox="0 0 20 20"
					fill="none"
					xmlns="http://www.w3.org/2000/svg"
				>
					<path class="arrow-up" d="M16 13L10 17L4 13" stroke="black" stroke-width="1.5" />
					<path class="arrow-down" d="M16 7L10 3L4 7" stroke="black" stroke-width="1.5" />
				</svg>
			</div>
		</div>

		<div class="download-wrapper-footer">
			<span>Open Beta {$latestClientVersion}</span>
			<div class="avaliable-os">
				<img
					class="os-icon"
					src="/images/os-icons/apple-small-logo.svg"
					alt="apple logo"
					title="Available on macOS"
				/>
				<img
					class="os-icon"
					src="/images/os-icons/linux-small-logo.svg"
					alt="linux logo"
					title="Available on Linux"
				/>
				<img
					class="os-icon"
					src="/images/os-icons/windows-small-logo.svg"
					alt="windows logo"
					title="Available on Windows"
				/>
			</div>
		</div>
	</div>

	{#if secondButton}
		<div class="second-button-wrap">
			{#if showVideoDemo}
				<a
					href={jsonLinks.other['youtube-demo'].url}
					target="_blank"
					class="yt-button-preview"
					onmouseenter={() => {
						videoElement?.play();
					}}
					onmouseleave={() => {
						videoElement?.pause();
						if (videoElement) {
							videoElement.currentTime = 0;
						}
					}}
				>
					<img class="yt-button-preview__logo-btn" src="/images/video-thumb/yt-logo.svg" alt="" />

					<video
						bind:this={videoElement}
						class="yt-button-preview__video"
						loop
						muted
						playsinline
						preload="auto"
						src="/images/video-thumb/video-thumb-loop.mp4#t=0.1"
					></video>
				</a>
			{/if}

			<a
				class="second-button"
				class:btn-blue={!lightTheme}
				class:btn-white={lightTheme}
				href={secondButton.url}
			>
				<span>
					{secondButton.label}
				</span>
				<svg
					class="second-button__arrow"
					width="20"
					height="20"
					viewBox="0 0 20 20"
					xmlns="http://www.w3.org/2000/svg"
				>
					<path
						fill-rule="evenodd"
						clip-rule="evenodd"
						d="M17.1884 1.75H0.99908V0.25H19.7491V19H18.2491V2.81066L1.52941 19.5303L0.46875 18.4697L17.1884 1.75Z"
					/>
				</svg>
			</a>
		</div>
	{/if}
</section>

<style lang="scss">
	.wrapper {
		display: flex;
		flex-direction: column;
		gap: 16px;
		user-select: none;
	}

	.download-wrapper {
		display: flex;
		position: relative;
		flex-direction: column;
	}

	.download-wrapper-head {
		display: flex;
		position: relative;

		&:hover {
			.divider {
				opacity: 0;
			}
		}

		@media (max-width: 500px) {
			flex-direction: column-reverse;
		}
	}

	.download-btn {
		display: flex;
		flex: 1;
		align-items: center;
		justify-content: center;

		height: 100%;
		padding: 16px 24px;
		border-radius: 12px 0 0 0;

		color: var(--clr-black);
		text-decoration: none;
		cursor: pointer;

		span {
			font-weight: 500;
			font-size: 28px;
			text-transform: uppercase;
		}

		@media (max-width: 500px) {
			border-radius: 0;

			span {
				font-size: 36px;
			}
		}
	}

	.arrow-up,
	.arrow-down {
		transition: transform 0.1s ease-in-out;
	}

	.select-btn {
		display: flex;
		position: relative;
		align-items: center;
		justify-content: center;
		height: auto;
		padding: 16px 24px;
		gap: 8px;
		border-radius: 0 12px 0 0;
		cursor: pointer;

		span {
			font-weight: 500;
			font-size: 16px;
			text-transform: uppercase;
		}

		&:hover {
			.arrow-up {
				transform: translateY(-1px);
			}

			.arrow-down {
				transform: translateY(1px);
			}
		}

		@media (max-width: 500px) {
			padding: 16px 20px;
			border-radius: 12px 12px 0 0;
		}
	}

	.divider {
		position: absolute;
		top: 20%;
		left: 0;
		width: 1px;
		width: 1px;
		height: 60%;
		border-radius: 0 12px 0 0;
		background-color: rgba(0, 0, 0, 0.16);
		transition: opacity 0.1s ease-in-out;

		@media (max-width: 500px) {
			top: auto;
			bottom: 0;
			width: 100%;
			height: 1px;
		}
	}

	.download-wrapper-footer {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 18px 24px;
		gap: 16px;
		border-right: 1px solid rgba(0, 0, 0, 0.16);
		border-bottom: 1px solid rgba(0, 0, 0, 0.16);
		border-left: 1px solid rgba(0, 0, 0, 0.16);
		border-radius: 0 0 12px 12px;
		user-select: none;

		span {
			font-weight: 500;
			font-size: 16px;
			line-height: 1;
			text-transform: uppercase;
			opacity: 0.5;
		}
	}

	.avaliable-os {
		display: flex;
		align-items: center;
		gap: 8px;
		gap: 8px;
	}

	.os-icon {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 22px;
		height: 22px;
		padding: 0;
		border: none;
		background: none;
		opacity: 0.5;

		&:hover,
		&:focus {
			border: none;
			outline: none;
			background: none;
		}
	}

	//////////////
	// SELECTOR //
	//////////////

	.os-select {
		display: flex;
		z-index: 10;

		position: absolute;
		right: 0;
		bottom: calc(100% + 4px);
		flex-direction: column;
		padding: 16px;
		border: 1px solid var(--clr-gray);
		border-radius: 12px;
		background-color: var(--clr-white);
		box-shadow: 0px 10px 8px rgba(0, 0, 0, 0.1);
	}

	.os-select__section {
		display: flex;
		gap: 8px;
	}

	.os-select__subsection {
		display: flex;
		flex-direction: column;
		width: 100%;
		gap: 2px;
	}

	.os-select__section-os-icon {
		width: 20px;
		height: 20px;
		margin-top: 4px;
		opacity: 0.8;
	}

	.os-select__item {
		display: flex;
		align-items: center;
		justify-content: space-between;
		width: 100%;
		padding: 6px;
		gap: 12px;
		border: none;
		border-radius: 6px;
		background: none;
		color: var(--clr-black);
		font-weight: 500;
		font-size: 16px;
		text-decoration: none;
		text-transform: uppercase;
		transition: background-color 0.05s ease-in-out;

		&:hover,
		&:focus-within {
			background-color: var(--clr-light-gray);
		}
	}

	.os-select__divider {
		width: calc(100% - 6px);
		height: 1px;
		margin: 12px 0;
		margin-left: 6px;
		border-bottom: 1px dashed rgba(0, 0, 0, 0.16);
	}

	////////////////
	// SECOND BTN //
	////////////////

	.second-button-wrap {
		display: flex;
		gap: 12px;

		@media (max-width: 500px) {
			flex-direction: column-reverse;
		}
	}

	////////////////////

	.yt-button-preview {
		display: flex;
		position: relative;
		align-items: center;
		justify-content: center;
		width: 130px;
		overflow: hidden;
		border-radius: 16px;
		background-color: rgba(0, 0, 0, 0.1);

		@media (max-width: 500px) {
			display: none;
		}
	}

	.yt-button-preview__logo-btn {
		z-index: 1;
		position: absolute;
		top: 50%;
		left: 50%;
		transform: translate(-50%, -50%);
	}

	.yt-button-preview__video {
		z-index: 0;
		position: absolute;
		top: 50%;
		left: 50%;
		width: 100%;
		transform: translate(-50%, -50%);
	}

	////////////////////

	.second-button {
		display: flex;
		flex: 1;
		align-items: center;
		justify-content: center;
		padding: 20px 24px;
		gap: 8px;
		border-radius: 12px;
		color: var(--clr-black);
		text-decoration: none;
		text-transform: uppercase;
		transition: background-color 0.1s ease-in-out;

		span {
			flex: 1;
			font-weight: 500;
			font-size: 28px;
			text-align: center;
			text-transform: uppercase;
		}

		&:hover,
		&:focus-within {
			.second-button__arrow {
				transform: translate(4px, -4px);
			}
		}
	}

	.second-button__arrow {
		transition: transform 0.1s ease-in-out;
	}

	.btn-blue {
		background-color: var(--clr-accent);
		transition: background-color 0.1s ease-in-out;

		&:hover {
			background-color: color-mix(in srgb, var(--clr-accent) 95%, var(--clr-black));
		}
	}

	.btn-white {
		background-color: var(--clr-white);
		transition: background-color 0.1s ease-in-out;

		&:hover {
			background-color: var(--clr-light-gray);
		}
	}
</style>
