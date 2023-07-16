<script lang="ts">
	import { SectionType } from './fileSections';
	import type { Line } from './fileSections';
	export let line: Line;
	export let sectionType: SectionType;
</script>

<span
	class="w-[1.5rem] select-none border-r border-light-400 bg-light-100 px-1 text-right text-light-600 dark:border-dark-400 dark:bg-dark-800 dark:text-light-300"
>
	{line.beforeLineNumber || ''}
</span>
<span
	class="w-[1.5rem] select-none border-r border-light-400 bg-light-100 px-1 text-right text-light-600 dark:border-dark-400 dark:bg-dark-800 dark:text-light-300"
>
	{line.afterLineNumber || ''}
</span>
<span
	class="pl-1 overflow-hidden whitespace-nowrap"
	class:diff-line-deletion={sectionType === SectionType.RemovedLines}
	class:diff-line-addition={sectionType === SectionType.AddedLines}
>
	{line.content}
</span>
