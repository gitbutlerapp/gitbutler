<script lang="ts">
	import { toHumanBranchName } from '$lib/branch';

	export let startTime: Date;
	export let endTime: Date;
	export let label: string;
	export let href: string;

	const timeToGridRow = (time: Date) => {
		const hours = time.getHours();
		const minutes = time.getMinutes();
		const totalMinutes = hours * 60 + minutes;
		const totalMinutesPerDay = 24 * 60;
		const gridRow = Math.floor((totalMinutes / totalMinutesPerDay) * 96);
		return gridRow + 1; // offset the first row
	};

	const dateToGridCol = (date: Date) => {
		return date.getDay();
	};

	const timeToSpan = (startTime: Date, endTime: Date) => {
		const startMinutes = startTime.getHours() * 60 + startTime.getMinutes();
		const endMinutes = endTime.getHours() * 60 + endTime.getMinutes();
		const span = Math.round((endMinutes - startMinutes) / 15); // 4 spans per hour
		if (span < 1) {
			return 1;
		} else {
			return span;
		}
	};
</script>

<li
	class="relative mt-px flex col-start-{dateToGridCol(startTime)}"
	style="grid-row: {timeToGridRow(startTime)} / span {timeToSpan(startTime, endTime)};"
>
	<a
		{href}
		title={startTime.toLocaleTimeString()}
		class="group absolute inset-1 flex flex-col items-center justify-center rounded-lg bg-zinc-300 p-3 leading-5 hover:bg-zinc-200 shadow"
	>
		<p class="order-1 font-semibold text-zinc-800">
			{toHumanBranchName(label)}
		</p>
	</a>
</li>
