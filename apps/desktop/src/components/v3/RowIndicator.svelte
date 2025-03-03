<script lang="ts">
	import Badge from '@gitbutler/ui/Badge.svelte';

	interface Props {
		first?: boolean;
		last?: boolean;
		content?: string;
	}

	const { first, last, content = 'Your commit goes here' }: Props = $props();
</script>

<div class="indicator" class:first class:last>
	<div class="pin">
		<div class="pin__line"></div>
		<div class="pin__circle"></div>
	</div>
	<div>
		<Badge size="tag" style="pop">{content}</Badge>
	</div>
</div>

<style>
	.indicator {
		padding: 12px 0;
		display: flex;
		gap: 12px;
		align-items: center;
		background-color: var(--clr-bg-1);
		&.first,
		&.last {
			border-top: 1px solid var(--clr-border-2);
		}
		&.last {
			border-bottom: none;
			border-radius: 0 0 var(--radius-l) var(--radius-l);
		}
	}
	.pin {
		display: flex;
		align-items: center;
		width: 40px;
		height: 10px;
		margin-left: -15px;
		position: relative;
	}
	.pin__line {
		flex-grow: 1;
		height: 2px;
		background-color: var(--clr-theme-pop-element);
	}
	.pin__circle {
		border-radius: 100%;
		width: 10px;
		height: 10px;
		outline: 2px solid var(--clr-theme-pop-element);
	}
</style>
