import { splitDiffIntoHunks } from '$lib/diffParsing';
import { describe, it, expect } from 'vitest';

describe('splitDiffIntoHunks', () => {
	it('parses hunks which contain @@ twice', () => {
		const diff = `diff --git a/butler/app/models/diff_cache.rb b/butler/app/models/diff_cache.rb
index A..B 100644
--- a/butler/app/models/diff_cache.rb
+++ b/butler/app/models/diff_cache.rb
@@ -70,14 +70,30 @@ class DiffCache < ApplicationRecord
 
     # read bytes until we hit the first line starting with \\"@@\\"
     # this is the start of the first hunk
+    eof_reached = false
     while (patch[current, 2] != \\"@@\\") && (patch[current, 12] != 'Binary files')
-      if patch[current] == \\"\\n\\"
+      # If we have reached the end of the file before we have hit a header (@@)
+      # then we can assume that we have encountered something like a 100% match
+      # rename. In this case, we want to make sure the contents of the last
+      # line are still added to the header_lines
+      if patch[current] == \\"\\n\\" || eof_reached
         header_lines << current_line
         current_line = \\"\\"
       else
         current_line += patch[current]
       end
-      current += 1
+
+      if eof_reached
+        break
+      end
+
+      # We don't want to let current to be >= patch.size, so we don't increment
+      # it when we reach EOF
+      if current + 1 == patch.size
+        eof_reached = true
+      else
+        current += 1
+      end
     end
 
     # identify the header lines`;

		expect(splitDiffIntoHunks(diff)).toStrictEqual([
			`@@ -70,14 +70,30 @@ class DiffCache < ApplicationRecord
 
     # read bytes until we hit the first line starting with \\"@@\\"
     # this is the start of the first hunk
+    eof_reached = false
     while (patch[current, 2] != \\"@@\\") && (patch[current, 12] != 'Binary files')
-      if patch[current] == \\"\\n\\"
+      # If we have reached the end of the file before we have hit a header (@@)
+      # then we can assume that we have encountered something like a 100% match
+      # rename. In this case, we want to make sure the contents of the last
+      # line are still added to the header_lines
+      if patch[current] == \\"\\n\\" || eof_reached
         header_lines << current_line
         current_line = \\"\\"
       else
         current_line += patch[current]
       end
-      current += 1
+
+      if eof_reached
+        break
+      end
+
+      # We don't want to let current to be >= patch.size, so we don't increment
+      # it when we reach EOF
+      if current + 1 == patch.size
+        eof_reached = true
+      else
+        current += 1
+      end
     end
 
     # identify the header lines`
		]);
	});
});
