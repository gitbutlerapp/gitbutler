<script lang="ts">
	import Tooltip from '$lib/Tooltip.svelte';
	import Icon from '$lib/Icon.svelte';

	const props = $props();
</script>

<div class="wrapper">
	<p class="text-13 text">
		hello world! Here is a <Tooltip text={props.text} align={props.align} position={props.position}>
			<span class="tooltip-text">tooltip</span>
		</Tooltip> for you.
	</p>
</div>

<div class="menu">
	<Tooltip text={props.text} align={props.align} position={props.position}>
		<button class="menu-btn">
			<Icon name="info" />
		</button>
	</Tooltip>

	<Tooltip text={props.text} align={props.align} position={props.position}>
		<button class="menu-btn">
			<Icon name="plus" />
		</button>
	</Tooltip>

	<Tooltip text={props.text} align={props.align} position={props.position}>
		<button class="menu-btn">
			<Icon name="ai" />
		</button>
	</Tooltip>
</div>

<style>
	.wrapper {
		display: flex;
		flex-direction: column;
		padding: 20px;
	}

	.text {
		color: var(--clr-text-1);
	}

	.tooltip-text {
		text-decoration: underline;
		text-decoration-style: dotted;
	}

	.menu {
		transform: translateX(220px);
		display: flex;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		width: fit-content;
	}

	.menu-btn {
		padding: 4px;
	}
</style>
