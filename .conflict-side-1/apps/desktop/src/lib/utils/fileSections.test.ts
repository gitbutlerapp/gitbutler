import { LocalFile } from '$lib/files/file';
import { Hunk } from '$lib/hunks/hunk';
import { parseHunkSection, parseFileSections, SectionType } from '$lib/utils/fileSections';
import { plainToInstance } from 'class-transformer';
import { expect, test } from 'vitest';
import type { ContentSection, HunkSection } from '$lib/utils/fileSections';

const fileContent = `<!DOCTYPE html>
<html lang="en">
	<head>
		<meta charset="utf-8" />
		<link rel="icon" href="%sveltekit.assets%/favicon.png" />
		<meta name="viewport" content="width=device-width" />
		%sveltekit.head%
	</head>
	<body data-sveltekit-preload-data="hover" class="bg-[#212124] text-zinc-400">
		<div style="display: contents">%sveltekit.body%</div>
	</body>
</html>`;
const balancedHunkDiff = `@@ -3,7 +3,7 @@
        <head>
                <meta charset="utf-8" />
                <link rel="icon" href="%sveltekit.assets%/favicon.png" />
-               <meta name="viewport" content="width=device-width" />
+               <meta name="viewporttt" content="width=device-width" />
                %sveltekit.head%
        </head>
        <body data-sveltekit-preload-data="hover" class="bg-[#212124] text-zinc-400">`;
const moreAddedHunkDiff = `@@ -3,7 +3,8 @@
        <head>
                <meta charset="utf-8" />
                <link rel="icon" href="%sveltekit.assets%/favicon.png" />
-               <meta name="viewport" content="width=device-width" />
+               <meta name="viewport"
+                       content="width=device-width" />
                %sveltekit.head%
        </head>
        <body data-sveltekit-preload-data="hover" class="bg-[#212124] text-zinc-400">`;
const topOfFileHunk = `@@ -1,5 +1,5 @@
 <!DOCTYPE html>
-<html lang="en">
+<html lang="de">
        <head>
                <meta charset="utf-8" />
                <link rel="icon" href="%sveltekit.assets%/favicon.png" />`;
const bottomOfFileHunk = `@@ -8,5 +8,6 @@
        </head>
        <body data-sveltekit-preload-data="hover" class="bg-[#212124] text-zinc-400">
                <div style="display: contents">%sveltekit.body%</div>
+               <p>wtf</p>
        </body>
 </html>`;
const delteWholeFile = `@@ -1,12 +0,0 @@
-<!DOCTYPE html>
-<html lang="en">
-       <head>
-               <meta charset="utf-8" />
-               <link rel="icon" href="%sveltekit.assets%/favicon.png" />
-               <meta name="viewport" content="width=device-width" />
-               %sveltekit.head%
-       </head>
-       <body data-sveltekit-preload-data="hover" class="bg-[#212124] text-zinc-400">
-               <div style="display: contents">%sveltekit.body%</div>
-       </body>
-</html>`;
const addWholeFile = `@@ -0,0 +1,12 @@
+<!DOCTYPE html>
+<html lang="en">
+       <head>
+               <meta charset="utf-8" />
+               <link rel="icon" href="%sveltekit.assets%/favicon.png" />
+               <meta name="viewport" content="width=device-width" />
+               %sveltekit.head%
+       </head>
+       <body data-sveltekit-preload-data="hover" class="bg-[#212124] text-zinc-400">
+               <div style="display: contents">%sveltekit.body%</div>
+       </body>
+</html>`;
const multiChangeHunk = `@@ -1,12 +1,11 @@
 <!DOCTYPE html>
 <html lang="en">
        <head>
-               <meta charset="utf-8" />
                <link rel="icon" href="%sveltekit.assets%/favicon.png" />
                <meta name="viewport" content="width=device-width" />
                %sveltekit.head%
        </head>
-       <body data-sveltekit-preload-data="hover" class="bg-[#212124] text-zinc-400">
+       <body data-sveltekit-preload-data="Hover" class="bg-[#212124] text-zinc-400">
                <div style="display: contents">%sveltekit.body%</div>
        </body>
 </html>`;
const conflictMarkersHunk = `@@ -3,7 +3,11 @@
 	<head>
 		<meta charset="utf-8" />
 		<meta name="viewport" content="width=device-width" />
-		%sveltekit.head%
+<<<<<<< ours
+		%sveltekit.HEAD%
+=======
+		%Sveltekit.head%
+>>>>>>> theirs
 	</head>

 	<body`;

test('handle broken diffs', () => {
	const balancedHunk = plainToInstance(Hunk, {
		id: '1',
		diff: 'not a real diff',
		modifiedAt: new Date(2021, 1, 1),
		filePath: 'foo.py',
		locked: false
	});
	const hunkSection = parseHunkSection(balancedHunk);
	expect(hunkSection).toBeDefined();
	expect(hunkSection.subSections.length).toBe(0);
});

test('parses a balanced hunk section', () => {
	const balancedHunk = plainToInstance(Hunk, {
		id: '1',
		diff: balancedHunkDiff,
		modifiedAt: new Date(2021, 1, 1),
		filePath: 'foo.py',
		locked: false
	});
	const hunkSection = parseHunkSection(balancedHunk);
	expect(hunkSection).toBeDefined();
	expect(hunkSection?.hasConflictMarkers).toBe(false);
	expect(hunkSection?.hunk).toStrictEqual(balancedHunk);
	expect(hunkSection?.header.beforeStart).toBe(3);
	expect(hunkSection?.header.beforeLength).toBe(7);
	expect(hunkSection?.header.afterStart).toBe(3);
	expect(hunkSection?.header.afterLength).toBe(7);
	expect(hunkSection.subSections.length).toBe(4);
	const firstContext = hunkSection.subSections[0];
	expect(firstContext?.sectionType).toBe(SectionType.Context);
	expect(firstContext?.expanded).toBeTruthy();
	expect(firstContext?.lines).toEqual([
		{
			beforeLineNumber: 3,
			afterLineNumber: 3,
			content: `       <head>`
		},
		{
			beforeLineNumber: 4,
			afterLineNumber: 4,
			content: `               <meta charset="utf-8" />`
		},
		{
			beforeLineNumber: 5,
			afterLineNumber: 5,
			content: `               <link rel="icon" href="%sveltekit.assets%/favicon.png" />`
		}
	]);
	const removedLines = hunkSection.subSections[1];
	expect(removedLines?.sectionType).toBe(SectionType.RemovedLines);
	expect(removedLines?.expanded).toBeTruthy();
	expect(removedLines?.lines).toEqual([
		{
			beforeLineNumber: 6,
			afterLineNumber: undefined,
			content: `               <meta name="viewport" content="width=device-width" />`
		}
	]);
	const addedLines = hunkSection.subSections[2];
	expect(addedLines?.sectionType).toBe(SectionType.AddedLines);
	expect(addedLines?.expanded).toBeTruthy();
	expect(addedLines?.lines).toEqual([
		{
			beforeLineNumber: undefined,
			afterLineNumber: 6,
			content: `               <meta name="viewporttt" content="width=device-width" />`
		}
	]);
	const secondContext = hunkSection.subSections[3];
	expect(secondContext?.sectionType).toBe(SectionType.Context);
	expect(secondContext?.expanded).toBeTruthy();
	expect(secondContext?.lines).toEqual([
		{
			beforeLineNumber: 7,
			afterLineNumber: 7,
			content: `               %sveltekit.head%`
		},
		{
			beforeLineNumber: 8,
			afterLineNumber: 8,
			content: `       </head>`
		},
		{
			beforeLineNumber: 9,
			afterLineNumber: 9,
			content: `       <body data-sveltekit-preload-data="hover" class="bg-[#212124] text-zinc-400">`
		}
	]);
});

test('parses a hunk with conflict markers', () => {
	const balancedHunk = plainToInstance(Hunk, {
		id: '1',
		diff: conflictMarkersHunk,
		modifiedAt: new Date(2021, 1, 1),
		filePath: 'foo.py',
		locked: false
	});
	const hunkSection = parseHunkSection(balancedHunk);
	expect(hunkSection).toBeDefined();
	expect(hunkSection?.hunk).toStrictEqual(balancedHunk);
	expect(hunkSection?.hasConflictMarkers).toBe(true);
});

test('parses hunk sections with more added', () => {
	const balancedHunk = plainToInstance(Hunk, {
		id: '1',
		diff: moreAddedHunkDiff,
		modifiedAt: new Date(2021, 1, 1),
		filePath: 'foo.py',
		locked: false
	});
	const hunkSection = parseHunkSection(balancedHunk);
	expect(hunkSection).toBeDefined();
	expect(hunkSection?.hunk).toStrictEqual(balancedHunk);
	expect(hunkSection?.header.beforeStart).toBe(3);
	expect(hunkSection?.header.beforeLength).toBe(7);
	expect(hunkSection?.header.afterStart).toBe(3);
	expect(hunkSection?.header.afterLength).toBe(8);
	expect(hunkSection.subSections.length).toBe(4);
	const firstContext = hunkSection.subSections[0];
	expect(firstContext?.sectionType).toBe(SectionType.Context);
	expect(firstContext?.expanded).toBeTruthy();
	expect(firstContext?.lines).toEqual([
		{
			beforeLineNumber: 3,
			afterLineNumber: 3,
			content: `       <head>`
		},
		{
			beforeLineNumber: 4,
			afterLineNumber: 4,
			content: `               <meta charset="utf-8" />`
		},
		{
			beforeLineNumber: 5,
			afterLineNumber: 5,
			content: `               <link rel="icon" href="%sveltekit.assets%/favicon.png" />`
		}
	]);

	const removedLines = hunkSection.subSections[1];
	expect(removedLines?.sectionType).toBe(SectionType.RemovedLines);
	expect(removedLines?.expanded).toBeTruthy();
	expect(removedLines?.lines).toEqual([
		{
			beforeLineNumber: 6,
			afterLineNumber: undefined,
			content: `               <meta name="viewport" content="width=device-width" />`
		}
	]);

	const addedLines = hunkSection.subSections[2];
	expect(addedLines?.sectionType).toBe(SectionType.AddedLines);
	expect(addedLines?.expanded).toBeTruthy();
	expect(addedLines?.lines).toEqual([
		{
			beforeLineNumber: undefined,
			afterLineNumber: 6,
			content: `               <meta name="viewport"`
		},
		{
			beforeLineNumber: undefined,
			afterLineNumber: 7,
			content: `                       content="width=device-width" />`
		}
	]);

	const secondContext = hunkSection.subSections[3];
	expect(secondContext?.sectionType).toBe(SectionType.Context);
	expect(secondContext?.expanded).toBeTruthy();
	expect(secondContext?.lines).toEqual([
		{
			beforeLineNumber: 7,
			afterLineNumber: 8,
			content: `               %sveltekit.head%`
		},
		{
			beforeLineNumber: 8,
			afterLineNumber: 9,
			content: `       </head>`
		},
		{
			beforeLineNumber: 9,
			afterLineNumber: 10,
			content: `       <body data-sveltekit-preload-data="hover" class="bg-[#212124] text-zinc-400">`
		}
	]);
});

test('parses a hunk with two changed places', () => {
	const balancedHunk = plainToInstance(Hunk, {
		id: '1',
		diff: multiChangeHunk,
		modifiedAt: new Date(2021, 1, 1),
		filePath: 'foo.py',
		locked: false
	});
	const hunkSection = parseHunkSection(balancedHunk);
	expect(hunkSection).toBeDefined();
	expect(hunkSection?.hunk).toStrictEqual(balancedHunk);
	expect(hunkSection?.header.beforeStart).toBe(1);
	expect(hunkSection?.header.beforeLength).toBe(12);
	expect(hunkSection?.header.afterStart).toBe(1);
	expect(hunkSection?.header.afterLength).toBe(11);
	expect(hunkSection.subSections.length).toBe(6);

	const firstContext = hunkSection.subSections[0];
	expect(firstContext?.sectionType).toBe(SectionType.Context);
	expect(firstContext?.expanded).toBeTruthy();
	expect(firstContext?.lines).toEqual([
		{
			beforeLineNumber: 1,
			afterLineNumber: 1,
			content: `<!DOCTYPE html>`
		},
		{
			beforeLineNumber: 2,
			afterLineNumber: 2,
			content: `<html lang="en">`
		},
		{
			beforeLineNumber: 3,
			afterLineNumber: 3,
			content: `       <head>`
		}
	]);
	const firstHunkSubsection = hunkSection.subSections[1];
	expect(firstHunkSubsection?.sectionType).toBe(SectionType.RemovedLines);
	expect(firstHunkSubsection?.expanded).toBeTruthy();
	expect(firstHunkSubsection?.lines).toEqual([
		{
			beforeLineNumber: 4,
			afterLineNumber: undefined,
			content: `               <meta charset="utf-8" />`
		}
	]);
	const secondContext = hunkSection.subSections[2];
	expect(secondContext?.sectionType).toBe(SectionType.Context);
	expect(secondContext?.expanded).toBeTruthy();
	expect(secondContext?.lines).toEqual([
		{
			beforeLineNumber: 5,
			afterLineNumber: 4,
			content: `               <link rel="icon" href="%sveltekit.assets%/favicon.png" />`
		},
		{
			beforeLineNumber: 6,
			afterLineNumber: 5,
			content: `               <meta name="viewport" content="width=device-width" />`
		},
		{
			beforeLineNumber: 7,
			afterLineNumber: 6,
			content: `               %sveltekit.head%`
		},
		{
			beforeLineNumber: 8,
			afterLineNumber: 7,
			content: `       </head>`
		}
	]);
	const secondHunkSubsection = hunkSection.subSections[3];
	expect(secondHunkSubsection?.sectionType).toBe(SectionType.RemovedLines);
	expect(secondHunkSubsection?.expanded).toBeTruthy();
	expect(secondHunkSubsection?.lines).toEqual([
		{
			beforeLineNumber: 9,
			afterLineNumber: undefined,
			content: `       <body data-sveltekit-preload-data="hover" class="bg-[#212124] text-zinc-400">`
		}
	]);
	const thirdHunkScubsection = hunkSection.subSections[4];
	expect(thirdHunkScubsection?.sectionType).toBe(SectionType.AddedLines);
	expect(thirdHunkScubsection?.expanded).toBeTruthy();
	expect(thirdHunkScubsection?.lines).toEqual([
		{
			beforeLineNumber: undefined,
			afterLineNumber: 8,
			content: `       <body data-sveltekit-preload-data="Hover" class="bg-[#212124] text-zinc-400">`
		}
	]);
	const thirdContext = hunkSection.subSections[5];
	expect(thirdContext?.sectionType).toBe(SectionType.Context);
	expect(thirdContext?.expanded).toBeTruthy();
	expect(thirdContext?.lines).toEqual([
		{
			beforeLineNumber: 10,
			afterLineNumber: 9,
			content: `               <div style="display: contents">%sveltekit.body%</div>`
		},
		{
			beforeLineNumber: 11,
			afterLineNumber: 10,
			content: `       </body>`
		},
		{
			beforeLineNumber: 12,
			afterLineNumber: 11,
			content: `</html>`
		}
	]);
});

test('parses file with one hunk and balanced add-remove', () => {
	const hunk = plainToInstance(Hunk, {
		id: '1',
		diff: balancedHunkDiff,
		modifiedAt: new Date(2021, 1, 1),
		filePath: 'foo.py',
		locked: false
	});
	const file = plainToInstance(LocalFile, {
		id: '1',
		path: 'foo.py',
		hunks: [hunk],
		expanded: true,
		modifiedAt: new Date(2021, 1, 1),
		conflicted: false,
		content: fileContent,
		binary: false
	});
	const sections = parseFileSections(file);
	expect(sections.length).toBe(3);
	const beforeSection = sections[0] as ContentSection;
	expect(beforeSection.sectionType).toBe(SectionType.Context);
	expect(beforeSection.expanded).toBeTruthy();
	expect(beforeSection.lines.length).toBe(2);
	expect(beforeSection.lines[0]).toEqual({
		beforeLineNumber: 1,
		afterLineNumber: 1,
		content: '<!DOCTYPE html>'
	});
	expect(beforeSection.lines[1]).toEqual({
		beforeLineNumber: 2,
		afterLineNumber: 2,
		content: '<html lang="en">'
	});

	const hunkSection = sections[1] as HunkSection;
	expect(hunkSection.hunk).toStrictEqual(hunk);

	const afterSection = sections[2] as ContentSection;
	expect(afterSection.sectionType).toBe(SectionType.Context);
	expect(afterSection.expanded).toBeTruthy();
	expect(afterSection.lines.length).toBe(3);
	expect(afterSection.lines[0]).toEqual({
		beforeLineNumber: 10,
		afterLineNumber: 10,
		content: '		<div style="display: contents">%sveltekit.body%</div>'
	});
	expect(afterSection.lines[1]).toEqual({
		beforeLineNumber: 11,
		afterLineNumber: 11,
		content: '	</body>'
	});
	expect(afterSection.lines[2]).toEqual({
		beforeLineNumber: 12,
		afterLineNumber: 12,
		content: '</html>'
	});
});

test('parses file with one hunk with more added than removed', () => {
	const hunk = plainToInstance(Hunk, {
		id: '1',
		diff: moreAddedHunkDiff,
		modifiedAt: new Date(2021, 1, 1),
		filePath: 'foo.py',
		locked: false
	});
	const file = plainToInstance(LocalFile, {
		id: '1',
		path: 'foo.py',
		hunks: [hunk],
		expanded: true,
		modifiedAt: new Date(2021, 1, 1),
		conflicted: false,
		content: fileContent,
		binary: false
	});
	const sections = parseFileSections(file);
	expect(sections.length).toBe(3);
	const beforeSection = sections[0] as ContentSection;
	expect(beforeSection.lines.length).toBe(2);
	expect(beforeSection.lines[0]).toEqual({
		beforeLineNumber: 1,
		afterLineNumber: 1,
		content: '<!DOCTYPE html>'
	});
	expect(beforeSection.lines[1]).toEqual({
		beforeLineNumber: 2,
		afterLineNumber: 2,
		content: '<html lang="en">'
	});

	const hunkSection = sections[1] as HunkSection;
	expect(hunkSection.hunk).toStrictEqual(hunk);

	const afterSection = sections[2] as ContentSection;
	expect(afterSection.lines.length).toBe(3);
	expect(afterSection.lines[0]).toEqual({
		beforeLineNumber: 10,
		afterLineNumber: 11,
		content: '		<div style="display: contents">%sveltekit.body%</div>'
	});
	expect(afterSection.lines[1]).toEqual({
		beforeLineNumber: 11,
		afterLineNumber: 12,
		content: '	</body>'
	});
	expect(afterSection.lines[2]).toEqual({
		beforeLineNumber: 12,
		afterLineNumber: 13,
		content: '</html>'
	});
});

test('parses file with two hunks ordered by position in file', () => {
	const topHunk = plainToInstance(Hunk, {
		id: '1',
		diff: topOfFileHunk,
		modifiedAt: new Date(2021, 1, 1),
		filePath: 'foo.py',
		locked: false
	});
	const bottomHunk = plainToInstance(Hunk, {
		id: '1',
		diff: bottomOfFileHunk,
		modifiedAt: new Date(2021, 1, 1),
		filePath: 'foo.py',
		locked: false
	});
	const file = plainToInstance(LocalFile, {
		id: '1',
		path: 'foo.py',
		hunks: [bottomHunk, topHunk],
		expanded: true,
		modifiedAt: new Date(2021, 1, 1),
		conflicted: false,
		content: fileContent,
		binary: false
	});
	const sections = parseFileSections(file);
	expect(sections.length).toBe(3);
	const topHunkSection = sections[0] as HunkSection;
	expect(topHunkSection.hunk).toStrictEqual(topHunk);
	const middleSection = sections[1] as ContentSection;
	expect(middleSection.lines.length).toBe(2);
	expect(middleSection.lines[0]).toEqual({
		beforeLineNumber: 6,
		afterLineNumber: 6,
		content: '		<meta name="viewport" content="width=device-width" />'
	});
	expect(middleSection.lines[1]).toEqual({
		beforeLineNumber: 7,
		afterLineNumber: 7,
		content: '		%sveltekit.head%'
	});
	const bottomHunkSection = sections[2] as HunkSection;
	expect(bottomHunkSection.hunk).toStrictEqual(bottomHunk);
	expect(bottomHunkSection.subSections[0]?.sectionType).toBe(SectionType.Context);
	expect(bottomHunkSection.subSections[0]?.lines[0]).toEqual({
		beforeLineNumber: 8,
		afterLineNumber: 8,
		content: '       </head>'
	});
	expect(bottomHunkSection.subSections[0]?.lines[1]).toEqual({
		beforeLineNumber: 9,
		afterLineNumber: 9,
		content: '       <body data-sveltekit-preload-data="hover" class="bg-[#212124] text-zinc-400">'
	});
	expect(bottomHunkSection.subSections[0]?.lines[2]).toEqual({
		beforeLineNumber: 10,
		afterLineNumber: 10,
		content: '               <div style="display: contents">%sveltekit.body%</div>'
	});
	expect(bottomHunkSection.subSections[1]?.sectionType).toBe(SectionType.AddedLines);
	expect(bottomHunkSection.subSections[1]?.lines.length).toBe(1);
	expect(bottomHunkSection.subSections[1]?.lines[0]).toEqual({
		beforeLineNumber: undefined,
		afterLineNumber: 11,
		content: '               <p>wtf</p>'
	});
	expect(bottomHunkSection.subSections[2]?.sectionType).toBe(SectionType.Context);
	expect(bottomHunkSection.subSections[2]?.lines.length).toBe(2);
	expect(bottomHunkSection.subSections[2]?.lines[0]).toEqual({
		beforeLineNumber: 11,
		afterLineNumber: 12,
		content: '       </body>'
	});
	expect(bottomHunkSection.subSections[2]?.lines[1]).toEqual({
		beforeLineNumber: 12,
		afterLineNumber: 13,
		content: '</html>'
	});
});

test('parses whole file deleted', () => {
	const deleteHunk = plainToInstance(Hunk, {
		id: '1',
		diff: delteWholeFile,
		modifiedAt: new Date(2021, 1, 1),
		filePath: 'foo.py',
		locked: false
	});
	const file = plainToInstance(LocalFile, {
		id: '1',
		path: 'foo.py',
		hunks: [deleteHunk],
		expanded: true,
		modifiedAt: new Date(2021, 1, 1),
		conflicted: false,
		content: fileContent,
		binary: false
	});
	const sections = parseFileSections(file);
	expect(sections.length).toBe(1);
	const deleteHunkSection = sections[0] as HunkSection;
	expect(deleteHunkSection.hunk).toStrictEqual(deleteHunk);
	expect(deleteHunkSection.subSections.length).toBe(1);
	expect(deleteHunkSection.subSections[0]?.sectionType).toBe(SectionType.RemovedLines);
	expect(deleteHunkSection.subSections[0]?.lines.length).toBe(12);
});

test('parses new file created', () => {
	const newFileHunk = plainToInstance(Hunk, {
		id: '1',
		diff: addWholeFile,
		modifiedAt: new Date(2021, 1, 1),
		filePath: 'foo.py',
		locked: false
	});
	const file = plainToInstance(LocalFile, {
		id: '1',
		path: 'foo.py',
		hunks: [newFileHunk],
		expanded: true,
		modifiedAt: new Date(2021, 1, 1),
		conflicted: false,
		content: fileContent,
		binary: false
	});
	const sections = parseFileSections(file);
	expect(sections.length).toBe(1);
	const deleteHunkSection = sections[0] as HunkSection;
	expect(deleteHunkSection.hunk).toStrictEqual(newFileHunk);
	expect(deleteHunkSection.subSections.length).toBe(1);
	expect(deleteHunkSection.subSections[0]?.sectionType).toBe(SectionType.AddedLines);
	expect(deleteHunkSection.subSections[0]?.lines.length).toBe(12);
});
