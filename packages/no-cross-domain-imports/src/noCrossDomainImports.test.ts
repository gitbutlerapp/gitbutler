import { noCrossDomainImports } from "./noCrossDomainImports.js";
import { RuleTester } from "eslint";
import { describe, it } from "vitest";

// RuleTester needs a describe/it pair — point it at vitest's.
RuleTester.describe = describe;
RuleTester.it = it;

const tester = new RuleTester({
	languageOptions: { ecmaVersion: 2022, sourceType: "module" },
});

tester.run("no-cross-domain-imports", noCrossDomainImports, {
	valid: [
		// Single domain — always fine
		{
			filename: "/project/src/components/commit/CommitView.svelte",
			code: `
				import A from "$components/commit/CommitDetails.svelte";
				import B from "$components/commit/CommitTitle.svelte";
			`,
		},
		// Two non-shared domains — at the default limit of 2
		{
			filename: "/project/src/components/branch/BranchCard.svelte",
			code: `
				import A from "$components/branch/BranchHeader.svelte";
				import B from "$components/forge/PrBadge.svelte";
			`,
		},
		// shared/ only — excluded by default, never counted
		{
			filename: "/project/src/components/commit/CommitView.svelte",
			code: `
				import A from "$components/shared/Drawer.svelte";
				import B from "$components/shared/ReduxResult.svelte";
			`,
		},
		// shared/ + one domain — still within limit
		{
			filename: "/project/src/components/commit/CommitView.svelte",
			code: `
				import A from "$components/commit/CommitDetails.svelte";
				import B from "$components/shared/Drawer.svelte";
				import C from "$components/shared/ReduxResult.svelte";
			`,
		},
		// views/ is exempt via composition layer (maxDomains: 99 simulates the ESLint override)
		{
			filename: "/project/src/components/views/StackView.svelte",
			code: `
				import A from "$components/branch/BranchHeader.svelte";
				import B from "$components/commit/CommitView.svelte";
				import C from "$components/diff/UnifiedDiffView.svelte";
				import D from "$components/files/FileList.svelte";
			`,
			options: [{ maxDomains: 99 }],
		},
		// Custom excludeDomains — editor/ excluded, so 2 real domains is fine
		{
			filename: "/project/src/components/commit/CommitMessageEditor.svelte",
			code: `
				import A from "$components/commit/CommitDetails.svelte";
				import B from "$components/editor/MessageEditor.svelte";
				import C from "$components/diff/UnifiedDiffView.svelte";
			`,
			options: [{ maxDomains: 2, excludeDomains: ["shared", "editor"] }],
		},
		// Non-$components imports are ignored entirely
		{
			filename: "/project/src/components/commit/CommitView.svelte",
			code: `
				import A from "@gitbutler/ui/Button.svelte";
				import B from "$lib/stores/commit";
				import C from "../relative/path";
			`,
		},
		// Exactly maxDomains with a custom higher limit
		{
			filename: "/project/src/components/foo/Foo.svelte",
			code: `
				import A from "$components/branch/X.svelte";
				import B from "$components/commit/Y.svelte";
				import C from "$components/diff/Z.svelte";
			`,
			options: [{ maxDomains: 3 }],
		},
	],

	invalid: [
		// Three distinct domains — exceeds default maxDomains of 2
		{
			filename: "/project/src/components/shared/GlobalModal.svelte",
			code: `
				import A from "$components/commit/AutoCommitModal.svelte";
				import B from "$components/onboarding/LoginConfirmation.svelte";
				import C from "$components/settings/AuthorMissing.svelte";
			`,
			errors: [{ message: /Imports from 3 \$components domains/ }],
		},
		// Four domains
		{
			filename: "/project/src/components/shared/BigComponent.svelte",
			code: `
				import A from "$components/branch/BranchHeader.svelte";
				import B from "$components/commit/CommitView.svelte";
				import C from "$components/diff/UnifiedDiffView.svelte";
				import D from "$components/files/FileList.svelte";
			`,
			errors: [{ message: /Imports from 4 \$components domains/ }],
		},
		// Three domains with shared/ present — shared excluded, still 3 non-shared
		{
			filename: "/project/src/components/foo/Foo.svelte",
			code: `
				import A from "$components/branch/X.svelte";
				import B from "$components/commit/Y.svelte";
				import C from "$components/diff/Z.svelte";
				import D from "$components/shared/Util.svelte";
			`,
			errors: [{ message: /Imports from 3 \$components domains/ }],
		},
		// Exceeds custom maxDomains: 1
		{
			filename: "/project/src/components/branch/BranchCard.svelte",
			code: `
				import A from "$components/branch/BranchHeader.svelte";
				import B from "$components/forge/PrBadge.svelte";
			`,
			options: [{ maxDomains: 1 }],
			errors: [{ message: /maximum is 1/ }],
		},
		// Error message lists domains alphabetically
		{
			filename: "/project/src/components/foo/Foo.svelte",
			code: `
				import A from "$components/zebra/X.svelte";
				import B from "$components/alpha/Y.svelte";
				import C from "$components/middle/Z.svelte";
			`,
			errors: [{ message: /alpha, middle, zebra/ }],
		},
		// Error message includes the current folder name
		{
			filename: "/project/src/components/commit/BigCommit.svelte",
			code: `
				import A from "$components/branch/X.svelte";
				import B from "$components/diff/Y.svelte";
				import C from "$components/files/Z.svelte";
			`,
			errors: [{ message: /This file is in commit\// }],
		},
		// Domain folder: error mentions views/ as a fix option
		{
			filename: "/project/src/components/commit/Composer.svelte",
			code: `
				import A from "$components/branch/X.svelte";
				import B from "$components/forge/Y.svelte";
				import C from "$components/diff/Z.svelte";
			`,
			errors: [{ message: /move this component to views\// }],
		},
		// shared/ file: tailored message warns that shared/ shouldn't import domains at all
		{
			filename: "/project/src/components/shared/Composer.svelte",
			code: `
				import A from "$components/branch/X.svelte";
				import B from "$components/commit/Y.svelte";
				import C from "$components/diff/Z.svelte";
			`,
			errors: [{ message: /shared\/ is a utilities tier/ }],
		},
		// shared/ file: does NOT show the generic "domain folders" guidance
		{
			filename: "/project/src/components/shared/Composer.svelte",
			code: `
				import A from "$components/branch/X.svelte";
				import B from "$components/commit/Y.svelte";
				import C from "$components/diff/Z.svelte";
			`,
			errors: [{ message: /^(?!.*domain folders should only)/ }],
		},
	],
});
