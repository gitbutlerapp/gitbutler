# Hunk dependency

This crate contains the logic behind the calculation of the dependency graph between commits and between uncommited changes.

## Overview

### Define dependent
We define a patch being 'dependent' on another patch if it cannot be applied on top of another one, without leading to a merge conflict of the stacks.\
The stacks (formerly known as virtual branches) need to be independently mergeable to main and with each other.

Dependency also plays a role in the integrity of stacks themselves.\
It should be impossible to move a commit that introduces file A, above a commit that modifies it.

All that being said, probably there is some flexibility in what 'dependency' means in this context, especially in a world of *fearless rebasing*.\
We should play around with the limits of what should be rearrangeable.

### What do we want?

1. Be able to tell which uncommited changes belong to which stacks.
2. Be able to tell which commits depend on which preceding commits.
3. Be able to tell which commits are depended upon by following commits or uncommited changes (i.e. the inverse map of the previous two items).

### What do we need?

In order to calculate the things above, we need a way of telling if a given file change (patch) depends on a preceding one.\
The way we calculate this needs to be performant, because of the high frequency at which this is called.

### Approach

@mtsgrd came up with the idea of keeping an ordered list of line ranges, that each contain information about what commit introduced them.\
Every time we process a new commit, we update the list, amending the ranges that were overwritten by the commit.

This results in a data structure that contains information about which lines in a file are 'owned' by which commit.\
But also which commits depend on previous commits. E.g. the commit that updates line 4 depends on the commit that introduced line 4.

### How do we get it?

In a nutshell, we iterate over the commits in application order (base to top of stack) and merge the diff hunks (by path) into a one-dimensional vector, representing line changes (hunk ranges).

The resulting vector can be thought of as the git-blame of a file.\
The beginning of the vector represents the top of the file, and the end of the vector the bottom.\
The items in the vector, i.e. hunk ranges, have the following information:
- Starting line of the range.
- Number of lines.
- Commit ID that introduced the change.
- Stack ID that owns the commit.
- Type of change (added file, deleted file, file modification).
- Net lines added.

## Algorithm's pseudo code (roughly)

What we're rougly doing in the code is
```
for all affected files:
  for all input diff hunks:
    if existing hunk is a deleted file
      add all diffs (should be one, recreating the file)
      break

    if input diff is deleting file
      override all previous hunks
      break

    if there are no exsting hunks
      add all diffs (can be multiple)
      break

    find the existing hunks that intersect by the input diff
    -> (visit all hunks after the last added hunk
        and before the hunk that follows the input diff)

    if any found:
      **update them to account for the change**
    else:
      insert it after the last existing hunk visited
      and shift the lines of all following existing hunks as needed
```

The more 'complex' part of this computation is handling all the ways that updating the intersecting hunks are dealt with.

## 'Special' hunks
At the beginning of the loop, we account for the cases in which the hunk is **completely deleting** or **creating a file**.
Those two operations are considered special because they can come only in a specific order and because they override the hunks completely.

The file creation can only appear when the list of hunk ranges is empty or when the only hunk range in the list is the deletion of a file (file is being recreated).

The file deletion can appear at any point as long as the existing range is not a deletion. If the incoming hunk is a file deletion, it has to be the only hunk in a commit.
The file deletion hunk overrides the complete list of existing hunk ranges.

## Updating intersecting hunks

When we get a list of hunks that are affected,
we only care about:
1. how the first hunk in the list is affected
2. how the last hunk in the list is affected (if there is more than one hunk affected)
3. how many lines were shifted

We **don't** care about the hunks in the middle of the list (if any),
since we can assume that they are all covered, and hence, overridden.

### Updating intersecting hunks
---

Intersecting hunks have their start and lines updated, depending on what the incoming hunk does.
We catch the 'special' cases at the top, so if hunk ranges intersect, it has to be file modification hunks.

A modification hunk can be only one of three:
1. Only adds lines
2. Only deletes lines
3. Adds and deletes lines (modifies lines)

Depending on how many hunk ranges are intersected and how, we update their `start` and `lines`.
After that, we filter out all ranges that have 0 `lines`, as this are the result of deletions and don't intersect anything anymore.

### All together now
---

Simply put, what we do is:

- Swap the hunks if they are completely overwritten by the incoming hunk.
- Trim the hunks that are partially overwritten by the incoming hunk.
- Update the `start` and `lines` of the intersecting hunk ranges.
- Filter out all ranges that contain 0 `lines`.
- Update all the starting lines of all following hunk ranges to account for the introduced line-shift.

## Shifting lines

Every modification hunk introduces a number of **net lines**.
Net lines is the number of added lines minus the number of removed lines.
Every incoming hunk shifts the starting line of the **following** hunk ranges by the net lines.
Preceding hunks are not shifted.

### Which hunks need to be updated?
---
Every time that we process an incoming hunk, we keep track of to indices of the hunk range list:
1. The index of the next hunk range to visit.
2. The index of the first hunk to shift.

The former is used when processing the following incoming hunk. We know that all hunks before the added hunk won't intersect with the next hunk, so we can skip to the one after the previously added.

The latter is used to shift the `start` of the following hunk ranges.

These two indices **can** be the same but don't have to.
An example in which they are not the same, is when an incoming hunk splits an existing hunk range:
Let the existing hunk range list be:
```
[
  HunkRange {
    start: 1
    lines: 10
  }
  HunkRange {
    start: 11
    lines: 1
  }
]
```

Let the incoming hunk be an update of line 3 (net lines = 0):

```
old_start: 3
old_lines: 1
new_start: 3
new_lines: 1
```

After processing the incoming hunk, the hunk range list looks like this
```
[
  HunkRange {
    start: 1
    lines: 2
  },
  HunkRange { <-- this is the introduced hunk
    start: 3
    lines: 1
  },
  HunkRange {
    start: 4
    lines: 7
  },
   HunkRange {
    start: 11
    lines: 1
  }
]
```
The existing range was split, and the incoming hunk split the existing range into the first and the last one.
The index of the next hunk range to visit is `2` and the index of the first hunk to shift is `3`.
When we process a following hunk, we need to compare it to the **3rd hunk range** (and following hunk ranges if any), but we need to shift the `start` lines of the hunk ranges following the ones we updated, in this case the **4th hunk range**.
Since the net lines are 0, everythin stays as is.

### Multiple shifts per commit
---

Inside a commit, multple hunks will most probably shift the lines different amounts.
We keep track of the cumulative line shift, and update only the hunks that follow the hunk that introduced the shift.

### Matching incoming hunks to shifted ranges
---

While determining whether an income hunk intersects an existing hunk range, we need to match the old start and old lines to the range.
The old start is supposed to match the file lines **before** shifting, so we need to take into account that while matching.
The `get_shifted_old_start` method takes care of that.

## Commit interdependency

The commit dependency graph is calculated as we create the hunk ranges list.

Every time, an incoming hunk introduced by **commit A** intersects with an existing one (or many) previously introduced by **commit B** (**C**, **D**, **...**), we consider the **commit A** dependent on **commit B** (**C**, **D**, **...**).

If an incoming hunk introduced by **commit A** does not intersect with any existing range, e.g. it's a new line at the bottom of the file, it's only dependent on the commit that created the file in the first place.
