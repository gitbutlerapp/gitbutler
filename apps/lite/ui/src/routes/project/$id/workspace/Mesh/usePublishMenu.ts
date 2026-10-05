import { Toast } from "@base-ui/react";
import { useQuery } from "@tanstack/react-query";
import { useHostedBranchPublish, useHostedBranchSend, useHostedFollow } from "#ui/api/mutations.ts";
import {
	guiSettingsQueryOptions,
	hostedMachinesQueryOptions,
	hostedPresenceQueryOptions,
} from "#ui/api/queries.ts";
import { type NativeMenuItem, nativeMenuItem, nativeMenuSeparator } from "#ui/native-menu.ts";
import { defaultSettings } from "#ui/settings.ts";
import type { PublishState } from "@gitbutler/but-sdk";

/**
 * Publishing and sending a branch of this machine's, as menu items, or none where publishing
 * isn't on. Uncommitted changes belong to a branch only in its own worktree, so only there
 * can they go too.
 */
export const usePublishMenu = ({
	projectId,
	branch,
	local,
	inWorktree,
	state,
	followed,
}: {
	projectId: string;
	branch: string;
	/** Whether this machine has the branch; another machine's isn't this one's to publish. */
	local: boolean;
	inWorktree: boolean;
	/** How it compares with what this machine last published; null if it never did. */
	state: PublishState | null;
	/** Whether it's published as it changes. */
	followed: boolean;
}): Array<NativeMenuItem> => {
	const { data: hostedBranches = false } = useQuery({
		...guiSettingsQueryOptions,
		select: (cfg) => cfg.hostedBranches ?? defaultSettings.hostedBranches,
	});
	const enabled = local && hostedBranches && window.lite.hosted !== true;
	const toastManager = Toast.useToastManager();
	const { isPending: isPublishPending, mutate: publish } = useHostedBranchPublish(projectId);
	const { isPending: isSendPending, mutate: send } = useHostedBranchSend(projectId);
	const { isPending: isFollowPending, mutate: follow } = useHostedFollow(projectId);
	// Every machine known to have this project or to be online; one offline gets it on return.
	const { data: published = [] } = useQuery({ ...hostedMachinesQueryOptions(projectId), enabled });
	const { data: online = [] } = useQuery(hostedPresenceQueryOptions(projectId));
	if (!enabled) return [];

	const recipients = [
		...new Set([...published.map((machine) => machine.name), ...online]),
	].toSorted();
	const done = { onSuccess: (message: string) => toastManager.add({ title: message }) };
	const publishItem = (label: string, includeUncommitted: boolean) =>
		nativeMenuItem({
			label,
			enabled: !isPublishPending,
			onSelect: () => publish({ projectId, branch, includeUncommitted }, done),
		});
	const sendItem = (label: string, includeUncommitted: boolean) =>
		nativeMenuItem({
			label,
			enabled: !isSendPending && recipients.length > 0,
			submenu: recipients.map((to) =>
				nativeMenuItem({
					label: to,
					onSelect: () => send({ projectId, branch, to, includeUncommitted }, done),
				}),
			),
		});

	// Publishing replaces what was published either way; said where that drops published commits.
	const publishBranch =
		state?.type === "published"
			? nativeMenuItem({ label: "Published", enabled: false })
			: state?.type === "diverged" || state?.type === "behind"
				? publishItem("Publish Branch (Replaces Published)", false)
				: publishItem("Publish Branch", false);

	return [
		publishBranch,
		sendItem("Send Branch To", false),
		...(inWorktree
			? [
					publishItem("Publish Branch With Uncommitted Changes", true),
					sendItem("Send Branch With Uncommitted Changes To", true),
				]
			: []),
		nativeMenuItem({
			label: inWorktree ? "Auto-Publish With Uncommitted Changes" : "Auto-Publish",
			checked: followed,
			enabled: !isFollowPending,
			onSelect: () => follow({ projectId, machine: null, branch, on: !followed }),
		}),
		nativeMenuSeparator,
	];
};
