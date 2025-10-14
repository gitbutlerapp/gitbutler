<script module lang="ts">
	export type faceType = 'idle' | 'thinking' | 'waiting' | 'compacted';
</script>

<script lang="ts">
	interface Props {
		mode?: faceType;
	}

	const { mode = 'idle' }: Props = $props();
</script>

<div class="but-face-wrapper {mode}">
	{#if mode === 'compacted'}
		<svg width="30" height="28" viewBox="0 0 30 28" fill="none" xmlns="http://www.w3.org/2000/svg">
			<path
				d="M0.999023 8.41602C0.999023 5.10231 3.68532 2.41602 6.99902 2.41602H18.3988C21.7125 2.41602 24.3988 5.10231 24.3988 8.41602V11.3811C24.3988 13.1339 25.1653 14.799 26.4967 15.9389L27.2547 16.5879C28.1424 17.3478 28.6533 18.4579 28.6533 19.6264V22.9357C28.6533 25.1449 26.8625 26.9357 24.6533 26.9357H6.99903C3.68532 26.9357 0.999023 24.2495 0.999023 20.9357V8.41602Z"
				fill="#FFD997"
				stroke="#D6A756"
				stroke-width="1.2"
			/>
			<path
				d="M9 2.43555H16V13.4355C16 14.5401 15.1046 15.4355 14 15.4355H11C9.89543 15.4355 9 14.5401 9 13.4355V2.43555Z"
				fill="white"
				stroke="#7985A7"
				stroke-width="1.2"
			/>
			<path d="M9 11.6022H12.5V15.4355H10.1667L9 13.7689V11.6022Z" fill="#7985A7" />
			<path d="M9 7.26888H12.5V9.43555H9V7.26888Z" fill="#7985A7" />
			<path d="M9 2.93555H12.5V5.10221H9V2.93555Z" fill="#7985A7" />
			<path d="M12.5 9.43555H16V11.6022H12.5V9.43555Z" fill="#7985A7" />
			<path d="M12.5 5.10221H16V7.26888H12.5V5.10221Z" fill="#7985A7" />
			<path
				d="M13.9189 13.3262C14.7271 13.3262 15.4057 13.9342 15.4951 14.7373L16.0352 19.5947C16.2692 21.7015 14.6198 23.5449 12.5 23.5449C10.3804 23.5447 8.73175 21.7014 8.96582 19.5947L9.50488 14.7373C9.59426 13.9341 10.2739 13.3262 11.082 13.3262H13.9189ZM12.501 18.7354C11.9488 18.7354 11.5011 19.1832 11.501 19.7354C11.501 20.2876 11.9487 20.7354 12.501 20.7354C13.0531 20.7352 13.501 20.2875 13.501 19.7354C13.5009 19.1833 13.053 18.7355 12.501 18.7354Z"
				fill="white"
			/>
			<path
				d="M15.4951 14.7373L16.0914 14.671L16.0914 14.6709L15.4951 14.7373ZM16.0352 19.5947L16.6315 19.5285L16.6315 19.5284L16.0352 19.5947ZM12.5 23.5449L12.4999 24.1449H12.5V23.5449ZM8.96582 19.5947L9.56215 19.661L9.56216 19.6609L8.96582 19.5947ZM9.50488 14.7373L8.90856 14.6709L8.90854 14.6711L9.50488 14.7373ZM12.501 18.7354L12.5012 18.1354H12.501V18.7354ZM11.501 19.7354L10.901 19.7352V19.7354H11.501ZM12.501 20.7354L12.501 21.3354L12.5012 21.3354L12.501 20.7354ZM13.501 19.7354L14.101 19.7354L14.101 19.7352L13.501 19.7354ZM13.9189 13.3262V13.9262C14.421 13.9262 14.8432 14.304 14.8988 14.8037L15.4951 14.7373L16.0914 14.6709C15.9682 13.5644 15.0331 12.7262 13.9189 12.7262V13.3262ZM15.4951 14.7373L14.8988 14.8036L15.4388 19.661L16.0352 19.5947L16.6315 19.5284L16.0914 14.671L15.4951 14.7373ZM16.0352 19.5947L15.4388 19.661C15.6334 21.4125 14.262 22.9449 12.5 22.9449V23.5449V24.1449C14.9775 24.1449 16.9051 21.9906 16.6315 19.5285L16.0352 19.5947ZM12.5 23.5449L12.5001 22.9449C10.7383 22.9447 9.36754 21.4125 9.56215 19.661L8.96582 19.5947L8.36949 19.5285C8.09595 21.9903 10.0226 24.1447 12.4999 24.1449L12.5 23.5449ZM8.96582 19.5947L9.56216 19.6609L10.1012 14.8035L9.50488 14.7373L8.90854 14.6711L8.36948 19.5285L8.96582 19.5947ZM9.50488 14.7373L10.1012 14.8037C10.1568 14.3044 10.5794 13.9262 11.082 13.9262V13.3262V12.7262C9.9683 12.7262 9.03177 13.5638 8.90856 14.6709L9.50488 14.7373ZM11.082 13.3262V13.9262H13.9189V13.3262V12.7262H11.082V13.3262ZM12.501 18.7354V18.1354C11.6173 18.1354 10.9011 18.8519 10.901 19.7352L11.501 19.7354L12.101 19.7355C12.101 19.5144 12.2802 19.3354 12.501 19.3354V18.7354ZM11.501 19.7354H10.901C10.901 20.619 11.6173 21.3354 12.501 21.3354V20.7354V20.1354C12.2801 20.1354 12.101 19.9563 12.101 19.7354H11.501ZM12.501 20.7354L12.5012 21.3354C13.3843 21.3351 14.101 20.6191 14.101 19.7354H13.501H12.901C12.901 19.9559 12.7219 20.1353 12.5008 20.1354L12.501 20.7354ZM13.501 19.7354L14.101 19.7352C14.1008 18.8518 13.3844 18.1357 12.5012 18.1354L12.501 18.7354L12.5008 19.3354C12.7217 19.3354 12.9009 19.5148 12.901 19.7355L13.501 19.7354Z"
				fill="#7985A7"
			/>
		</svg>
	{:else}
		<svg
			class="but-face__star"
			width="20"
			height="20"
			viewBox="0 0 20 20"
			fill="none"
			xmlns="http://www.w3.org/2000/svg"
		>
			<path
				d="M17.7996 8.46101C19.3468 8.90307 19.3467 11.0959 17.7996 11.538L15.2798 12.2583C13.8183 12.6759 12.6757 13.8185 12.258 15.28L11.5385 17.7991C11.0964 19.3464 8.90356 19.3464 8.46149 17.7991L7.74195 15.28C7.32431 13.8185 6.18167 12.6759 4.72018 12.2583L2.20042 11.538C0.65327 11.0959 0.653197 8.90307 2.20042 8.46101L4.71949 7.74147C6.1812 7.32384 7.32363 6.18141 7.74126 4.71969L8.46149 2.19993C8.90385 0.653344 11.0962 0.653348 11.5385 2.19993L12.2587 4.71969C12.6764 6.18141 13.8188 7.32384 15.2805 7.74147L17.7996 8.46101Z"
				fill="#24B4AD"
				stroke="var(--clr-bg-1)"
				stroke-width="1.2"
			/>
		</svg>

		<svg
			class="but-face__pc"
			width="30"
			height="27"
			viewBox="0 0 30 27"
			fill="none"
			xmlns="http://www.w3.org/2000/svg"
		>
			<path
				d="M1 7.37793C1 4.06422 3.68629 1.37793 7 1.37793H18.4537C21.7674 1.37793 24.4537 4.06422 24.4537 7.37793V10.3954C24.4537 12.1462 25.2185 13.8097 26.5474 14.9496L27.3222 15.6142C28.2082 16.3741 28.718 17.4831 28.718 18.6503V22.0001C28.718 24.2093 26.9272 26.0001 24.718 26.0001H7C3.68629 26.0001 1 23.3138 1 20.0001V7.37793Z"
				fill="#F2F2DA"
				stroke="#C3C39F"
				stroke-width="1.2"
			/>
			<rect
				x="4.12891"
				y="4.45605"
				width="16.6801"
				height="18.4667"
				rx="4"
				fill="white"
				stroke="black"
				stroke-width="1.2"
			/>
			<!-- MOUTH -->
			{#if mode === 'thinking' || mode === 'idle'}
				<path
					class="but-face__mouth-smile"
					d="M7.55078 16.6064C11.269 19.1174 13.296 19.1056 16.9056 16.6064"
					stroke="black"
					stroke-width="1.2"
				/>
			{:else if mode === 'waiting'}
				<path d="M7.99902 18H13.999" stroke="black" stroke-width="1.2" />
			{/if}
			<!-- EYES -->
			<rect
				class="but-face__eye but-face__eye--left"
				x="8.30078"
				y="7.875"
				width="2.74194"
				height="6.57575"
				rx="1.37097"
				fill="black"
			/>
			<rect
				class="but-face__eye but-face__eye--right"
				x="13.9141"
				y="7.87695"
				width="2.74194"
				height="6.57575"
				rx="1.37097"
				fill="black"
			/>
		</svg>
	{/if}
</div>

<style lang="postcss">
	.but-face-wrapper {
		position: relative;
		flex-shrink: 0;
		width: 28px;
		height: 28px;
	}

	.but-face__pc {
		z-index: 0;
		position: absolute;
		bottom: 0;
		left: 0;
		transform-origin: bottom center;
	}

	.but-face__star {
		z-index: 1;
		position: absolute;
		top: -6px;
		right: -5px;
		transform: scale(0.5) rotate(-20deg);
		opacity: 0;
		transition:
			opacity 0.2s ease,
			transform 0.2s ease;
	}

	/* THINKING MODE */
	.but-face-wrapper.thinking {
		.but-face__star {
			transform: scale(1) rotate(0deg);
			animation: alternateStar 2.4s cubic-bezier(0.4, 0, 0.6, 1) infinite;
			opacity: 1;
		}

		.but-face__pc {
			animation: pcBounce 1.6s ease-in-out infinite;
		}
	}

	@keyframes alternateStar {
		0% {
			transform: scale(1) rotate(0deg);
		}
		20% {
			transform: scale(1.05) rotate(90deg);
		}
		40% {
			transform: scale(1) rotate(180deg);
		}
		45% {
			transform: scale(1.15) rotate(180deg);
		}
		50% {
			transform: scale(1.2) rotate(180deg);
		}
		55% {
			transform: scale(1.15) rotate(180deg);
		}
		60% {
			transform: scale(1) rotate(180deg);
		}
		80% {
			transform: scale(1.05) rotate(270deg);
		}
		100% {
			transform: scale(1) rotate(360deg);
		}
	}

	.but-face__eye {
		transform-origin: center;
		transition: transform 0.3s ease-in-out;
	}

	.but-face-wrapper.thinking {
		.but-face__eye {
			animation: eyeBlink 3.5s ease-in-out infinite;
		}
	}
	@keyframes pcBounce {
		0%,
		100% {
			transform: translateY(0) rotate(0deg) scale(1, 1);
		}
		15% {
			transform: translateY(-1px) rotate(2deg) scale(1.02, 0.98);
		}
		30% {
			transform: translateY(0) rotate(3deg) scale(0.98, 1.02);
		}
		45% {
			transform: translateY(-0.5px) rotate(1deg) scale(1.01, 0.99);
		}
		60% {
			transform: translateY(0) rotate(-1deg) scale(0.99, 1.01);
		}
		75% {
			transform: translateY(-0.2px) rotate(-2deg) scale(1.005, 0.995);
		}
		90% {
			transform: translateY(0) rotate(-0.5deg) scale(0.995, 1.005);
		}
	}

	@keyframes eyeBlink {
		0%,
		85%,
		100% {
			transform: scaleY(1);
		}
		92% {
			transform: scaleY(0.2) translateY(-5px);
		}
		95% {
			transform: scaleY(1);
		}
	}

	/* WAITING MODE */
	.but-face-wrapper.waiting {
		.but-face__eye {
			animation: eyeRoll 6s ease-in-out infinite;
		}
	}

	@keyframes eyeRoll {
		0%,
		30% {
			transform: translate(0, 0);
		}
		35% {
			transform: translate(1px, -1px);
		}
		60% {
			transform: translate(1px, -1px);
		}
		65% {
			transform: translate(0, 0);
		}
		100% {
			transform: translate(0, 0);
		}
	}
</style>
