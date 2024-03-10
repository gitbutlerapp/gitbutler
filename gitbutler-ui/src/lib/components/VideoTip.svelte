<script lang="ts">
	export let src: string;
	export let playing: boolean = false;

	let video: HTMLVideoElement;

	$: if (video) {
		if (playing) {
			video.play();
		} else {
			video.pause();
		}
	}
</script>

<video bind:this={video} class="video-tip" {src} controls={false} loop muted playsinline>
	<track kind="captions" />
</video>

<style>
	.video-tip {
		pointer-events: none;
		width: 80px;
		border-radius: var(--radius-m);
		border: 1px solid var(--clr-theme-container-outline-light);
	}
</style>
