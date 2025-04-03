<script lang="ts">
	import bannerImgSVG from '$lib/assets/supporters.svg?raw';
	import { openExternalUrl } from '$lib/utils/url';
</script>

<button
	type="button"
	class="banner"
	onclick={async () => await openExternalUrl('https://docs.gitbutler.com/community/supporters')}
>
	<div class="banner-img-wrap">
		{@html bannerImgSVG}
	</div>

	<div class="banner-content">
		<h4 class="banner-label text-14 text-bold">Thank you to all GitButler early supporters</h4>
		<i class="banner-arrow-wrap">
			<div class="banner-arrow-tail"></div>
			<svg
				viewBox="0 0 7 11"
				fill="none"
				xmlns="http://www.w3.org/2000/svg"
				class="banner-arrow-head"
			>
				<path
					d="M0.871094 1L5.00013 5.5L0.871094 10"
					stroke="currentColor"
					vector-effect="non-scaling-stroke"
				/>
			</svg>
		</i>
	</div>
</button>

<style>
	.banner {
		position: relative;
		display: flex;
		flex-direction: column;
		padding: 16px;
		border-radius: var(--radius-m);
		background-color: var(--clr-theme-pop-soft);

		&:hover {
			& .banner-arrow-wrap {
				width: 12px;
			}

			& .banner-arrow-tail {
				width: 90%;
			}
		}
	}

	.banner-content {
		display: inline;
		color: var(--clr-theme-pop-on-soft);
		text-align: left;
	}

	.banner-label {
		display: inline;
	}

	.banner-img-wrap {
		position: relative;
		height: 40px;
		width: 100%;
		/* background-color: #00000043; */

		& :global(svg) {
			bottom: 0;
			right: 0;
			position: absolute;
			width: 106px;
			height: auto;
		}
	}

	/* ARROW */

	.banner-arrow-wrap {
		position: relative;
		display: inline-flex;
		transform: translateY(2px);
		width: 8px;
		height: 12px;
		transition: width 0.2s;
	}

	.banner-arrow-tail {
		position: absolute;
		top: 50%;
		transform: translateY(-50%);
		left: 0;
		width: 0%;
		height: 1px;
		background-color: currentColor;
		transition: width 0.2s;
	}

	.banner-arrow-head {
		position: absolute;
		right: 0;
		top: 0;
		height: 100%;

		& path {
			stroke-width: 1.5px;
		}
	}
</style>
