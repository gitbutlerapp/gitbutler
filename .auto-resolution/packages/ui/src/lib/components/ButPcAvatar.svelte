<script lang="ts">
	interface Props {
		mode?: 'idle' | 'thinking';
	}

	const { mode = 'idle' }: Props = $props();
</script>

<div class="but-face-wrapper {mode}">
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
		<path
			d="M7.55078 16.6064C11.269 19.1174 13.296 19.1056 16.9056 16.6064"
			stroke="black"
			stroke-width="1.2"
		/>
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
</div>

<style lang="postcss">
	.but-face-wrapper {
		position: relative;
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

	/* EYE ANIMATIONS */
	.but-face__eye {
		transform-origin: center;
		transition: transform 0.3s ease-in-out;
	}

	.but-face-wrapper.thinking .but-face__eye {
		animation: eyeBlink 3.5s ease-in-out infinite;
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
</style>
