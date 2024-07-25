<script lang="ts">
	export let name: string | undefined;

	const colors = [
		'#E78D8D',
		'#62CDCD',
		'#EC90D2',
		'#7DC8D8',
		'#F1BC55',
		'#6B6B4C',
		'#9785DE',
		'#99CE63',
		'#636ECE',
		'#5FD2B0'
	];

	function nameToColor(name: string | undefined) {
		const trimmed = name?.replace(/\s/g, '');
		if (!trimmed) {
			return `linear-gradient(45deg, ${colors[0][0]} 15%, ${colors[0][1]} 90%)`;
		}

		const startHash = trimmed.split('').reduce((acc, char) => acc + char.charCodeAt(0), 0);
		return colors[startHash % colors.length];
	}

	function getFirstLetter(name: string | undefined) {
		return name ? name[0].toUpperCase() : '';
	}

	$: firstLetter = getFirstLetter(name);
</script>

<div class="project-avatar" style:background-color={nameToColor(name)}>
	<svg class="avatar-letter" viewBox="0 0 24 24">
		<text x="50%" y="54%" text-anchor="middle" alignment-baseline="middle">
			{firstLetter.toUpperCase()}
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
		font-family: 'Inter', sans-serif;
		font-weight: 800;
		font-size: 16px;
		line-height: 1;
		fill: var(--clr-core-ntrl-100);
	}
</style>
