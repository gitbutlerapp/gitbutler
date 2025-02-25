<script lang="ts">
	import * as jsonLinks from '$home/lib/data/links.json';
	import { clickOutside } from '$home/lib/hooks/clickOutside';
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

	function handleClickOutside(e: MouseEvent) {
		e.stopPropagation();
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
				use:clickOutside={handleClickOutside}
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
					<path class="arrow-top" d="M16 13L10 17L4 13" stroke="black" stroke-width="1.5" />
					<path class="arrow-bottom" d="M16 7L10 3L4 7" stroke="black" stroke-width="1.5" />
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
		user-select: none;
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.download-wrapper {
		position: relative;
		display: flex;
		flex-direction: column;
	}

	.download-wrapper-head {
		position: relative;
		display: flex;

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
		cursor: pointer;
		display: flex;
		flex: 1;
		align-items: center;
		justify-content: center;
		padding: 16px 24px;

		height: 100%;
		border-radius: 12px 0 0 0;

		color: var(--clr-black);
		text-decoration: none;

		span {
			font-size: 28px;
			font-weight: 500;
			text-transform: uppercase;
		}

		@media (max-width: 500px) {
			border-radius: 0;

			span {
				font-size: 36px;
			}
		}
	}

	.arrow-top,
	.arrow-bottom {
		transition: transform 0.1s ease-in-out;
	}

	.select-btn {
		position: relative;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 8px;
		height: auto;
		padding: 16px 24px;
		border-radius: 0 12px 0 0;

		span {
			font-size: 16px;
			font-weight: 500;
			text-transform: uppercase;
		}

		&:hover {
			.arrow-top {
				transform: translateY(-1px);
			}

			.arrow-bottom {
				transform: translateY(1px);
			}
		}

		@media (max-width: 500px) {
			border-radius: 12px 12px 0 0;
			padding: 16px 20px;
		}
	}

	.divider {
		position: absolute;
		width: 1px;
		left: 0;
		top: 20%;
		width: 1px;
		height: 60%;
		background-color: rgba(0, 0, 0, 0.16);
		border-radius: 0 12px 0 0;
		transition: opacity 0.1s ease-in-out;

		@media (max-width: 500px) {
			top: auto;
			bottom: 0;
			width: 100%;
			height: 1px;
		}
	}

	.download-wrapper-footer {
		user-select: none;
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 16px;
		padding: 18px 24px;
		border-left: 1px solid rgba(0, 0, 0, 0.16);
		border-right: 1px solid rgba(0, 0, 0, 0.16);
		border-bottom: 1px solid rgba(0, 0, 0, 0.16);
		border-radius: 0 0 12px 12px;

		span {
			font-size: 16px;
			font-weight: 500;
			opacity: 0.5;
			text-transform: uppercase;
			line-height: 1;
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
		opacity: 0.5;
		background: none;

		&:hover,
		&:focus {
			border: none;
			background: none;
			outline: none;
		}
	}

	//////////////
	// SELECTOR //
	//////////////

	.os-select {
		display: flex;
		z-index: 10;

		position: absolute;
		bottom: calc(100% + 4px);
		right: 0;
		flex-direction: column;
		border-radius: 12px;
		padding: 16px;
		border: 1px solid var(--clr-gray);
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
		gap: 2px;
		width: 100%;
	}

	.os-select__section-os-icon {
		width: 20px;
		height: 20px;
		opacity: 0.8;
		margin-top: 4px;
	}

	.os-select__item {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		width: 100%;
		padding: 6px;
		text-decoration: none;
		color: var(--clr-black);
		border-radius: 6px;
		font-size: 16px;
		font-weight: 500;
		text-transform: uppercase;
		border: none;
		background: none;
		transition: background-color 0.05s ease-in-out;

		&:hover,
		&:focus-within {
			background-color: var(--clr-light-gray);
		}
	}

	.os-select__divider {
		width: calc(100% - 6px);
		height: 1px;
		border-bottom: 1px dashed rgba(0, 0, 0, 0.16);
		margin: 12px 0;
		margin-left: 6px;
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
		position: relative;
		width: 130px;
		display: flex;
		align-items: center;
		justify-content: center;
		border-radius: 16px;
		overflow: hidden;
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
		transform: translate(-50%, -50%);
		width: 100%;
	}

	////////////////////

	.second-button {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 8px;
		padding: 20px 24px;
		border-radius: 12px;
		color: var(--clr-black);
		text-decoration: none;
		text-transform: uppercase;
		transition: background-color 0.1s ease-in-out;

		span {
			flex: 1;
			text-align: center;
			font-size: 28px;
			font-weight: 500;
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
</style>
