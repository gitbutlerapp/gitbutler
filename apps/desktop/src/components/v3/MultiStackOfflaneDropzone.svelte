<script lang="ts">
	import Dropzone from '$components/Dropzone.svelte';
	import { OutsideLaneDzHandler } from '$lib/stacks/dropHandler';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';
	import { preventTransitionOnMount } from '@gitbutler/ui/utils/preventTransitionOnMount';

	interface Props {
		projectId: string;
	}

	const { projectId }: Props = $props();

	const [stackService, uiState] = inject(StackService, UiState);
	const dzHandler = $derived(new OutsideLaneDzHandler(stackService, projectId, uiState));
</script>

<div class="hidden-dropzone">
	<Dropzone handlers={[dzHandler]}>
		{#snippet overlay({ hovered, activated })}
			<div class="hidden-dropzone__lane" class:activated class:hovered>
				<div class="hidden-dropzone__content">
					<svg
						use:preventTransitionOnMount
						class="hidden-dropzone__svg"
						xmlns="http://www.w3.org/2000/svg"
						width="72"
						height="97"
						viewBox="0 0 72 97"
						fill="none"
					>
						<g class="hidden-dropzone__svg__plus-list">
							<path
								opacity="0.2"
								d="M11.001 8C11.001 3.58172 14.5827 0 19.001 0L63.4681 0C67.8863 0 71.4681 3.58172 71.4681 8V61.5474C71.4681 65.9657 67.8863 69.5474 63.4681 69.5474L19.001 69.5474C14.5827 69.5474 11.001 65.9657 11.001 61.5474L11.001 8Z"
								fill="var(--clr-scale-ntrl-70)"
								vector-effect="non-scaling-stroke"
							/>
							<path
								d="M41.5 11V39M58 25L25 25"
								stroke="var(--clr-scale-ntrl-60)"
								stroke-width="1.2"
								vector-effect="non-scaling-stroke"
							/>
						</g>

						<path
							d="M21.2127 46.8965H44.3127L53.9998 56.211V78.9384H21.2127V46.8965Z"
							fill="var(--clr-bg-2)"
							stroke="var(--clr-scale-ntrl-60)"
							stroke-width="1.2"
							class="hidden-dropzone__svg__back-list"
						/>

						<g class="hidden-dropzone__svg__front-list">
							<path
								d="M1.50497 44.3601L22.6618 41.5033L35.4695 50.876L39.2821 79.111L6.78991 83.4985L1.50497 44.3601Z"
								fill="var(--clr-bg-2)"
							/>
							<path
								d="M22.7453 44.8167L23.8802 53.2214L33.3355 51.9446M1.50497 44.3601L6.78991 83.4985L39.2821 79.111L35.4695 50.876L22.6618 41.5033L1.50497 44.3601Z"
								stroke="var(--clr-scale-ntrl-60)"
								stroke-width="1.2"
							/>
						</g>

						<path
							class="hidden-dropzone__svg__hand"
							d="M41.9997 86.5001C44.2007 82.1104 44.0217 76.9117 43.7761 73.8577C43.5047 70.4829 41.1648 66.4266 37.737 69.2678C36.5291 67.094 33.1479 66.6115 31.2155 69.51C29.7662 67.3363 26.1426 67.8188 25.4177 71.4417C22.7606 69.0268 17.7185 72.7903 21.7946 78.2053C19.6679 77.8508 18.1823 78.8867 18.1823 81.1907C18.1825 83.3666 20.9921 87.8999 27.0114 90.051M41.9997 86.5001C43.2294 87.2229 43.8415 88.3634 43.8415 89.5305C43.8413 95.0899 28.1455 98.1316 26.6501 92.8977C26.3934 91.9991 26.4866 91.0088 27.0114 90.051M41.9997 86.5001C41.3269 86.1044 40.4692 85.887 39.4279 85.7361M27.0114 90.051C28.3757 87.5293 31.6494 86.4975 34.2717 85.9859M27.9289 76.0507C28.597 77.1005 29.1287 78.6956 29.3074 79.7726M33.246 74.9872C33.6838 76.0682 33.914 77.7958 33.914 78.8728M38.3449 74.442C38.6994 75.5054 38.6994 77.4549 38.563 78.5319M31.2737 92.4388C33.056 90.9806 36.9446 90.0084 39.272 90.5357"
							stroke="var(--clr-scale-ntrl-60)"
							fill="var(--clr-bg-2)"
							stroke-width="1.2"
							vector-effect="non-scaling-stroke"
						/>
					</svg>

					<p use:preventTransitionOnMount class="hidden-dropzone__label text-13 text-body">
						Drag and drop files<br />to create a new branch.
					</p>
				</div>
			</div>
		{/snippet}
	</Dropzone>
</div>

<style lang="postcss">
	.hidden-dropzone {
		/* pointer-events: none; */
		user-select: none;
		position: relative;
		display: flex;
		flex-direction: column;
		flex: 1;
		overflow: hidden;
	}

	.hidden-dropzone__lane {
		position: absolute;
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 10px;
		min-height: 240px;
		min-width: 240px;

		/* SVG ANIMATION */
		&.activated {
			& .hidden-dropzone__svg,
			.hidden-dropzone__content:after {
				opacity: 1;
				transition: opacity 0.1s;
			}

			& .hidden-dropzone__svg__plus-list,
			.hidden-dropzone__svg__back-list,
			.hidden-dropzone__svg__front-list,
			.hidden-dropzone__svg__hand {
				transform: unset;
			}

			& .hidden-dropzone__label {
				opacity: 1;
				transform: translateY(0);
			}
		}
		&.hovered {
			& .hidden-dropzone__svg,
			.hidden-dropzone__content:after {
				opacity: 1;
				transition: opacity 0.1s;
			}

			& .hidden-dropzone__svg__plus-list {
				transform: translateY(0) scale(1.2);
			}
			& .hidden-dropzone__svg__plus-list path:nth-child(1) {
				fill: var(--clr-scale-pop-60);
			}
			& .hidden-dropzone__svg__plus-list path:nth-child(2) {
				stroke: var(--clr-theme-pop-element);
			}
			& .hidden-dropzone__svg__back-list {
				transform: translateX(3px) rotate(-5deg);
			}
			& .hidden-dropzone__svg__front-list {
				transform: translateX(-5px) rotate(-5deg);
			}
			& .hidden-dropzone__svg__hand {
				transform: rotate(-5deg);
			}
		}
	}

	.hidden-dropzone__content {
		pointer-events: none;
		z-index: var(--z-ground);
		position: relative;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 10px;

		&:after {
			z-index: -1;
			content: '';
			width: 400px;
			height: 400px;
			position: absolute;
			top: calc(50% - 50px);
			left: 50%;
			transform: translate(-50%, -50%);
			border-radius: 100%;
			opacity: 0;
			background: radial-gradient(var(--clr-bg-2) 0%, oklch(from var(--clr-bg-2) l c h / 0) 70%);
			transition: opacity 0.1s;
		}
	}

	.hidden-dropzone__label {
		text-align: center;
		color: var(--clr-text-3);
		opacity: 0;
		transform: translateY(5px);
		transition:
			opacity 0.15s,
			transform 0.15s;
		will-change: opacity, transform;
	}

	/* SVG */
	.hidden-dropzone__svg {
		opacity: 0;
		overflow: visible;
		transition: opacity 0.15s;
		will-change: opacity;
	}
	.hidden-dropzone__svg__plus-list {
		transform: translateY(10px) scale(0.9);
		transform-origin: center;
		transition: transform 0.2s;
		will-change: transform;
	}
	.hidden-dropzone__svg__plus-list path {
		transition:
			stroke 0.2s,
			fill 0.2s;
		will-change: stroke, fill;
	}
	.hidden-dropzone__svg__back-list {
		transform: translateY(10px) rotate(10deg);
		transform-origin: center;
		transition: transform 0.15s;
		will-change: transform;
	}
	.hidden-dropzone__svg__front-list {
		transform: translateY(10px) translateX(10px) rotate(-5deg);
		transform-origin: center;
		transition: transform 0.15s;
		will-change: transform;
	}
	.hidden-dropzone__svg__hand {
		transform: translateY(10px) rotate(-5deg) scale(0.9);
		transform-origin: center;
		transition: transform 0.2s;
		will-change: transform;
	}
</style>
