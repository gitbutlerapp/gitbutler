<script lang="ts">
	interface Props {
		darkMode?: boolean;
	}

	const { darkMode = false }: Props = $props();

	let copied = $state(false);

	function handleCopy() {
		navigator.clipboard.writeText("curl -fsSL https://gitbutler.com/install.sh | sh");
		copied = true;
		setTimeout(() => {
			copied = false;
		}, 2000);
	}
</script>

<section class="cta-wrap" class:dark-mode={darkMode}>
	<button type="button" class="copy-button" class:copied onclick={handleCopy}>
		<h3>Get the But CLI</h3>
		<code>curl -fsSL https://gitbutler.com/install.sh | sh</code>

		<svg
			class="copy-button__tick-icon"
			width="32"
			height="29"
			viewBox="0 0 32 29"
			fill="none"
			xmlns="http://www.w3.org/2000/svg"
		>
			<path
				d="M20.8096 5.75H7C5.20507 5.75 3.75 7.20507 3.75 9V21C3.75 22.7949 5.20507 24.25 7 24.25H21C22.7949 24.25 24.25 22.7949 24.25 21V17.1055H25.75V21C25.75 23.6234 23.6234 25.75 21 25.75H7C4.37665 25.75 2.25 23.6234 2.25 21V9C2.25 6.37665 4.37665 4.25 7 4.25H20.8096V5.75ZM31.0498 5.51074L19.1787 18.2881C17.7582 19.8171 15.3621 19.8921 13.8486 18.4551L7.9834 12.8867L9.0166 11.7988L14.8818 17.3682C15.7899 18.2301 17.2269 18.1848 18.0791 17.2676L29.9502 4.48926L31.0498 5.51074Z"
				fill="var(--clr-core-safe-60)"
			/>
		</svg>

		<svg
			class="copy-button__term-icon"
			width="32"
			height="29"
			viewBox="0 0 32 29"
			fill="none"
			xmlns="http://www.w3.org/2000/svg"
		>
			<path
				d="M26.2529 1.1084C28.895 1.24209 30.9961 3.42628 30.9961 6.10156V23.1016L30.9893 23.3584C30.8599 25.9154 28.8099 27.9653 26.2529 28.0947L25.9961 28.1016H5.99609L5.73926 28.0947C3.18227 27.9653 1.13232 25.9154 1.00293 23.3584L0.996094 23.1016V6.10156C0.996094 3.34014 3.23467 1.10156 5.99609 1.10156H25.9961L26.2529 1.1084ZM5.99609 2.60156C4.0631 2.60156 2.49609 4.16857 2.49609 6.10156V23.1016C2.49609 25.0346 4.0631 26.6016 5.99609 26.6016H25.9961C27.9291 26.6016 29.4961 25.0346 29.4961 23.1016V6.10156C29.4961 4.16857 27.9291 2.60156 25.9961 2.60156H5.99609ZM24 21.75H16V20.25H24V21.75ZM16.4727 13.918L17.1895 14.5L16.4727 15.082L8.47266 21.582L7.52734 20.418L14.8105 14.5L7.52734 8.58203L8.47266 7.41797L16.4727 13.918Z"
				fill="var(--copy-icon-fill)"
			/>
		</svg>

		<svg
			class="copy-button__copy-icon"
			width="32"
			height="29"
			viewBox="0 0 32 29"
			fill="none"
			xmlns="http://www.w3.org/2000/svg"
		>
			<path
				d="M26 1.25C28.6234 1.25 30.75 3.37665 30.75 6V17C30.75 19.6234 28.6234 21.75 26 21.75H22.75V23C22.75 25.6234 20.6234 27.75 18 27.75H6C3.37665 27.75 1.25 25.6234 1.25 23V12C1.25 9.37665 3.37665 7.25 6 7.25H9.25V6C9.25 3.37665 11.3766 1.25 14 1.25H26ZM6 8.75C4.20507 8.75 2.75 10.2051 2.75 12V23C2.75 24.7949 4.20507 26.25 6 26.25H18C19.7949 26.25 21.25 24.7949 21.25 23V12C21.25 10.2051 19.7949 8.75 18 8.75H6ZM14 2.75C12.2051 2.75 10.75 4.20507 10.75 6V7.25H18C20.6234 7.25 22.75 9.37665 22.75 12V20.25H26C27.7949 20.25 29.25 18.7949 29.25 17V6C29.25 4.20507 27.7949 2.75 26 2.75H14Z"
				fill="var(--copy-icon-fill)"
			/>
		</svg>
	</button>

	<a class="docs-button" href="https://docs.gitbutler.com/cli-overview" target="_blank">
		<svg
			class="docs-button__book"
			width="38"
			height="31"
			viewBox="0 0 38 31"
			fill="none"
			xmlns="http://www.w3.org/2000/svg"
		>
			<path
				d="M32.5322 3.71777C34.3685 3.39503 36.0516 4.80838 36.0518 6.67285V22.7432L36.04 23.0137C35.9197 24.3512 34.9157 25.4607 33.5713 25.6973L19.5186 28.1699L19.2588 28.2041C19.0857 28.2192 18.9114 28.2192 18.7383 28.2041L18.4785 28.1699L4.42578 25.6973C3.08126 25.4607 2.07736 24.3513 1.95703 23.0137L1.94531 22.7432V6.67285C1.94546 4.80831 3.62847 3.39487 5.46484 3.71777L18.4785 6.00684C18.7363 6.05218 18.9988 6.06366 19.2588 6.04102L19.5186 6.00684L32.5322 3.71777ZM5.20508 5.19531C4.28692 5.03385 3.44546 5.74063 3.44531 6.67285V22.7432C3.44548 23.4712 3.96851 24.0936 4.68555 24.2197L18.25 26.6064V7.48926C18.2396 7.4875 18.2291 7.4862 18.2188 7.48438L5.20508 5.19531ZM34.5518 6.67285C34.5516 5.7407 33.7101 5.034 32.792 5.19531L19.7783 7.48438C19.769 7.48602 19.7593 7.4867 19.75 7.48828V26.6055L33.3115 24.2197C34.0285 24.0935 34.5516 23.4711 34.5518 22.7432V6.67285ZM32.1475 18.7354L22.1475 20.7354L21.8525 19.2646L31.8525 17.2646L32.1475 18.7354ZM16.1475 15.2646L15.8525 16.7354L5.85254 14.7354L6.14746 13.2646L16.1475 15.2646ZM32.1475 14.7354L22.1475 16.7354L21.8525 15.2646L31.8525 13.2646L32.1475 14.7354ZM16.1475 11.2646L15.8525 12.7354L5.85254 10.7354L6.14746 9.26465L16.1475 11.2646ZM32.1475 10.7354L22.1475 12.7354L21.8525 11.2646L31.8525 9.26465L32.1475 10.7354Z"
			/>
		</svg>

		<div class="flex items-center gap-16 justify-between">
			<span>View docs</span>
			<svg
				class="docs-button__arrow"
				width="22"
				height="22"
				viewBox="0 0 22 22"
				fill="none"
				xmlns="http://www.w3.org/2000/svg"
			>
				<path
					d="M13.5618 0.414062L20.5 10.9141M20.5 10.9141L13.5618 21.4141M20.5 10.9141H0"
					stroke-width="1.5"
				/>
			</svg>
		</div>
	</a>
</section>

<style lang="postcss">
	.cta-wrap {
		display: flex;
		margin-top: 40px;
		gap: 12px;

		/* Light mode defaults */
		--copy-bg: var(--clr-core-gray-20);
		--copy-shadow: inset 0px -36px 54px rgba(0, 0, 0, 0.2);
		--copy-text: white;
		--copy-code: var(--clr-core-pop-70);
		--copy-icon-fill: white;
		--copy-icon-opacity: 0.4;
		--docs-border: var(--clr-border-2);
		--docs-text: var(--clr-text-2);
		--docs-hover-bg: color-mix(in srgb, var(--clr-bg-1) 60%, transparent);

		&.dark-mode {
			--copy-bg: var(--clr-theme-pop-on-element);
			--copy-shadow: none;
			--copy-text: var(--clr-core-pop-40);
			--copy-code: var(--clr-core-pop-40);
			--copy-icon-fill: var(--clr-core-pop-40);
			--copy-icon-opacity: 0.5;
			--docs-border: var(--clr-core-pop-70);
			--docs-text: var(--clr-theme-pop-on-element);
			--docs-hover-bg: color-mix(in srgb, var(--clr-theme-pop-on-element) 10%, transparent);
		}
	}

	.copy-button {
		display: flex;
		position: relative;
		flex-direction: column;
		align-items: flex-start;
		padding: 24px 40px 24px 24px;
		gap: 6px;
		border-radius: var(--radius-xl);
		background: var(--copy-bg);
		box-shadow: var(--copy-shadow);
		cursor: pointer;
		transition: background 0.2s;

		& h3 {
			color: var(--copy-text);
			font-size: 48px;
			line-height: 1;
			font-family: var(--font-accent);
		}

		& code {
			color: var(--copy-code);
			font-size: 14px;
			opacity: 0.7;
		}

		&:hover {
			& .copy-button__term-icon {
				transform: scale(0.8);
				opacity: 0;
			}

			& .copy-button__copy-icon {
				transform: scale(1);
				opacity: 0.6;
			}
		}
	}

	.copy-button__term-icon,
	.copy-button__copy-icon,
	.copy-button__tick-icon {
		position: absolute;
		top: 20px;
		right: 24px;
		fill: var(--copy-icon-fill);
		opacity: var(--copy-icon-opacity);
		pointer-events: none;
		transition:
			opacity 0.1s,
			transform 0.2s;
	}

	.copy-button__copy-icon {
		transform: scale(0.8);
		opacity: 0;
	}

	.copy-button__tick-icon {
		transform: scale(0.8);
		opacity: 0;
	}

	.copy-button.copied {
		& .copy-button__term-icon,
		& .copy-button__copy-icon {
			transform: scale(0.8);
			opacity: 0;
		}

		& .copy-button__tick-icon {
			transform: scale(1);
			opacity: 1;
		}
	}

	.docs-button {
		display: flex;
		flex-direction: column;
		justify-content: space-between;
		min-width: 180px;
		padding: 20px;
		border: 1px solid var(--docs-border);
		border-radius: var(--radius-xl);
		background: none;
		transition: background 0.2s;

		& span {
			color: var(--docs-text);
		}

		&:hover {
			background: var(--docs-hover-bg);
		}

		&:hover .docs-button__arrow {
			transform: translateX(4px);
			opacity: 0.8;
		}
	}

	.docs-button__arrow {
		stroke: var(--docs-text);
		opacity: 0.5;
		transition:
			transform 0.2s,
			opacity 0.2s;
	}

	.docs-button__book {
		margin-bottom: 8px;
		fill: var(--docs-text);
	}

	@media (--tablet-viewport) {
		.copy-button code {
			font-size: 12px;
		}
	}

	@media (--mobile-viewport) {
		.cta-wrap {
			flex-direction: column;
			margin-top: 20px;
			gap: 12px;
		}
	}
</style>
