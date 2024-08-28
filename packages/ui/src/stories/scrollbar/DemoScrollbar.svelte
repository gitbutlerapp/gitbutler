<script lang="ts">
	import Scrollbar from '$lib/scroll/Scrollbar.svelte';

	const { ...args } = $props();

	let viewport = $state<HTMLDivElement | undefined>(undefined);
	let contents = $state<HTMLDivElement | undefined>(undefined);
</script>

<div class="wrapper">
	<div bind:this={viewport} class="wievport hide-native-scrollbar">
		<div bind:this={contents} class="content">
			<p class="text-14 text-body">
				Artificial Intelligence (AI) has emerged as one of the most transformative technologies of
				the 21st century, influencing various sectors, from healthcare and education to finance and
				entertainment. As AI systems become increasingly integrated into our daily lives, it’s
				crucial to understand their evolution, the opportunities they present, and the challenges
				they pose. This essay delves into the historical development of AI, its current
				applications, and its profound impact on society. By examining the benefits and risks
				associated with AI, we can better appreciate its role in shaping the future.
			</p>

			<p class="text-14 text-body">
				The early years of AI research were dominated by symbolic AI, also known as "Good
				Old-Fashioned Artificial Intelligence" (GOFAI). This approach was based on the idea that
				human intelligence could be represented by manipulating symbols and following rules.
				Researchers like McCarthy, Marvin Minsky, and Allen Newell focused on creating programs that
				could perform logical reasoning, problem-solving, and symbolic manipulation.
			</p>

			<p class="text-14 text-body">
				During the 1950s and 1960s, significant progress was made in areas like game playing,
				theorem proving, and natural language processing. For example, the Logic Theorist, developed
				by Newell and Simon in 1955, was one of the first AI programs capable of solving problems in
				symbolic logic. However, despite these achievements, early AI systems were limited in their
				ability to handle real-world complexity.
			</p>
		</div>

		<Scrollbar {viewport} {contents} {...args} />
	</div>
</div>

<style lang="postcss">
	.wrapper {
		position: relative;
		width: fit-content;
	}

	.wievport {
		/* position: relative; */
		display: flex;
		flex-direction: column;
		gap: 16px;
		width: 100%;
		max-width: 500px;
		max-height: 300px;
		overflow-y: scroll;
		color: var(--clr-text-1);
		background-color: rgba(0, 0, 0, 0.1);
	}

	.content {
		padding: 16px;
	}
</style>
