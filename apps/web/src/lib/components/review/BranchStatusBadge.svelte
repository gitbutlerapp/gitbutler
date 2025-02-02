<script lang="ts">
	import CommitStatusBadge from '@gitbutler/ui/CommitStatusBadge.svelte';
	import type { Branch, Patch } from '@gitbutler/shared/branches/types';

	type Props = {
		branch: Branch;
	};

	const { branch }: Props = $props();

	const patches = branch.patches;

	let anyRejected = patches.some((patch: Patch) => patch.reviewAll.rejected.length > 0);
	let someApproved = patches.some((patch: Patch) => patch.reviewAll.signedOff.length > 0);
	let allApproved = !patches.some((patch: Patch) => patch.reviewAll.signedOff.length === 0);

	function getStatus() {
		if (anyRejected) {
			return 'changes-requested';
		} else if (allApproved) {
			return 'approved';
		} else if (someApproved) {
			return 'in-discussion';
		} else {
			return 'unreviewed';
		}
	}
</script>

<CommitStatusBadge status={getStatus()} />
