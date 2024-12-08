<script lang="ts">
	export let label = 'Label';
	export let icon: 'github' | 'discord' | undefined = undefined;
	export let hideTextOnTablet = false;
	export let href = '#';
	export let hrefTarget = '_self';

	const icons = {
		github:
			'M10.0074 1.5C5.02656 1.5 1 5.39582 1 10.2155C1 14.0681 3.57996 17.3292 7.15904 18.4835C7.60652 18.5702 7.77043 18.2959 7.77043 18.0652C7.77043 17.8631 7.75568 17.1706 7.75568 16.449C5.25002 16.9685 4.72824 15.41 4.72824 15.41C4.32557 14.3999 3.72893 14.1403 3.72893 14.1403C2.90883 13.6064 3.78867 13.6064 3.78867 13.6064C4.69837 13.6642 5.17572 14.501 5.17572 14.501C5.98089 15.8285 7.27833 15.4534 7.8003 15.2225C7.87478 14.6597 8.11355 14.2701 8.36706 14.0537C6.36863 13.8517 4.26602 13.1014 4.26602 9.75364C4.26602 8.80129 4.6237 8.02213 5.19047 7.41615C5.10105 7.19976 4.7878 6.30496 5.28008 5.10735C5.28008 5.10735 6.04062 4.87643 7.75549 6.00197C9.22525 5.62006 10.7896 5.61702 12.2592 6.00197C13.9743 4.87643 14.7348 5.10735 14.7348 5.10735C15.2271 6.30496 14.9137 7.19976 14.8242 7.41615C15.4059 8.02213 15.7489 8.80129 15.7489 9.75364C15.7489 13.1014 13.6463 13.8372 11.6329 14.0537C11.9611 14.3279 12.2443 14.8472 12.2443 15.6698C12.2443 16.8385 12.2295 17.7765 12.2295 18.065C12.2295 18.2959 12.3936 18.5702 12.8409 18.4836C16.42 17.3291 19 14.0681 19 10.2155C19.0147 5.39582 14.9734 1.5 10.0074 1.5Z',
		discord:
			'M16.9419 3.75623C15.6279 3.16091 14.2407 2.73857 12.8158 2.5C12.6208 2.84494 12.4443 3.19983 12.2872 3.5632C10.7694 3.33686 9.22584 3.33686 7.70801 3.5632C7.55079 3.19987 7.37437 2.84498 7.17946 2.5C5.75361 2.74059 4.3655 3.16393 3.05016 3.75934C0.43887 7.5825 -0.269009 11.3107 0.0849305 14.986C1.61417 16.1041 3.32582 16.9544 5.14548 17.5C5.55522 16.9547 5.91778 16.3761 6.22933 15.7705C5.63759 15.5518 5.06646 15.282 4.52255 14.9642C4.6657 14.8615 4.8057 14.7556 4.94099 14.6529C6.52364 15.3894 8.25103 15.7713 9.99997 15.7713C11.7489 15.7713 13.4763 15.3894 15.0589 14.6529C15.1958 14.7634 15.3358 14.8692 15.4774 14.9642C14.9324 15.2825 14.3602 15.5529 13.7675 15.7721C14.0786 16.3774 14.4412 16.9555 14.8513 17.5C16.6725 16.9566 18.3855 16.1067 19.915 14.9875C20.3303 10.7254 19.2055 7.03144 16.9419 3.75623ZM6.67765 12.7257C5.69134 12.7257 4.87649 11.84 4.87649 10.7503C4.87649 9.66065 5.66302 8.76712 6.6745 8.76712C7.68599 8.76712 8.49454 9.66065 8.47724 10.7503C8.45993 11.84 7.68284 12.7257 6.67765 12.7257ZM13.3223 12.7257C12.3344 12.7257 11.5227 11.84 11.5227 10.7503C11.5227 9.66065 12.3092 8.76712 13.3223 8.76712C14.3353 8.76712 15.1376 9.66065 15.1203 10.7503C15.103 11.84 14.3275 12.7257 13.3223 12.7257Z'
	};
</script>

{#if $$slots['dropdown']}
	<div class="link link-dropdown" role="button" tabindex="0">
		<span> {label} </span>

		<svg
			class="dropdown-icon"
			width="12"
			height="16"
			viewBox="0 0 12 16"
			xmlns="http://www.w3.org/2000/svg"
		>
			<path
				class="dropdown-icon__arrow-top"
				d="M10.5156 10L6.01562 14L1.51562 10"
				stroke="black"
				stroke-width="1.5"
			/>
			<path
				class="dropdown-icon__arrow-bottom"
				d="M10.5156 6L6.01563 2L1.51562 6"
				stroke="black"
				stroke-width="1.5"
			/>
		</svg>

		<div class="dropdown-wrapper">
			<div class="dropdown-container">
				<slot name="dropdown" />
			</div>
		</div>
	</div>
{:else}
	<a class="link" {href} target={hrefTarget}>
		<span class:hide-on-tablet={hideTextOnTablet}> {label} </span>

		{#if icon}
			<svg class="icon" viewBox="0 0 20 20" xmlns="http://www.w3.org/2000/svg">
				<path fill-rule="evenodd" clip-rule="evenodd" d={icons[icon]} />
			</svg>
		{/if}
	</a>
{/if}

<style lang="scss">
	.link {
		user-select: none;
		position: relative;
		cursor: pointer;
		display: flex;
		align-items: center;
		padding: 10px 12px;
		border-radius: 8px;
		gap: 8px;
		font-size: 18px;
		font-weight: 500;
		color: var(--clr-black);
		text-decoration: none;
		text-transform: uppercase;
		background-color: transparent;
		transition: background-color 0.05s ease-in-out;

		&:hover {
			background-color: color-mix(in srgb, var(--clr-gray), var(--clr-white) 50%);
		}
	}

	.link-dropdown {
		&:hover {
			.dropdown-wrapper {
				opacity: 1;
				transform: translateY(0);
				pointer-events: auto;
			}
		}
	}

	.dropdown-wrapper {
		z-index: 10;
		position: absolute;
		right: 0;
		top: 100%;
		padding-top: 6px;

		pointer-events: none;
		opacity: 0;
		transform: translateY(-8px);

		transition:
			opacity 0.1s ease-in-out,
			transform 0.1s ease-in-out;
	}

	.dropdown-container {
		// display: none;
		position: relative;
		background-color: var(--clr-white);
		padding: 12px;
		border-radius: 12px;
		width: max-content;
		border: 1px solid var(--clr-gray);
		box-shadow: 0px 4px 8px rgba(0, 0, 0, 0.08);

		:global(a) {
			display: flex;
			gap: 16px;
			align-items: center;
			justify-content: space-between;
			padding: 8px;
			color: var(--clr-black);
			text-decoration: none;
			text-transform: uppercase;
			font-size: 16px;
			font-weight: 500;
			border-radius: 6px;
			transition: background-color 0.05s ease-in-out;

			&:hover {
				background-color: var(--clr-light-gray);

				:global(svg) {
					transform: scale(1);
					opacity: 1;
				}
			}
		}

		:global(svg) {
			opacity: 0;
			transform: scale(0.8);
			transition:
				opacity 0.05s ease-in-out,
				transform 0.2s ease-in-out;
		}
	}

	.dropdown-icon {
		fill: none;
	}

	.icon {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 20px;
		height: 20px;
	}

	.hide-on-tablet {
		@media (max-width: 1300px) {
			display: none;
		}
	}
</style>
