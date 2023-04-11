<script lang="ts">
	import { onMount } from 'svelte';
	import getMD5 from 'md5';

	export let email = '';
	let avatarImage: HTMLImageElement;

	onMount(() => {
		let hash = getMD5(email, { encoding: 'binary' });
		let src = `https://www.gravatar.com/avatar/${hash}?d=retro`;
		avatarImage.src = src;
	});
</script>

<img class="relative z-30 inline-block h-6 w-6 rounded-full" bind:this={avatarImage} alt={email} />
