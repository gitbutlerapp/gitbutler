<script lang="ts">
	import butAgentAvatarSvg from '$lib/assets/but-agent.svg?raw';
</script>

<div class="but-agent-avatar">
	{@html butAgentAvatarSvg}
</div>

<style lang="postcss">
	.but-agent-avatar {
		display: flex;
		position: relative;
		align-items: center;
		justify-content: center;
		width: 28px;
		height: 28px;
	}

	:global(.but-agent-avatar svg) {
		position: absolute;
		bottom: 0;
		left: 0;
	}
</style>
