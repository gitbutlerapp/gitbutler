<script lang="ts">
	import { stringToColor } from '@gitbutler/ui/utils/stringToColor';

	interface Props {
		name?: string;
	}

	const { name }: Props = $props();

	function getFirstLetter(name: string | undefined) {
		return name ? name[0]?.toUpperCase() : '';
	}

	const firstLetter = $derived(getFirstLetter(name));
</script>

<div class="project-avatar" style:background-color={stringToColor(name)}>
	<svg class="avatar-letter" viewBox="0 0 24 24">
		<text x="50%" y="54%" text-anchor="middle" alignment-baseline="middle">
			{firstLetter?.toUpperCase()}
		</text>
	</svg>
</div>

<style>
	.project-avatar {
		flex-shrink: 0;
		width: 20px;
		height: 20px;
		border-radius: var(--radius-m);
	}

	.avatar-letter {
		width: 100%;
		height: 100%;
	}

	.avatar-letter text {
		font-family: var(--fontfamily-default);
		font-weight: 800;
		font-size: 16px;
		line-height: 1;
		fill: var(--clr-core-ntrl-100);
	}
</style>
