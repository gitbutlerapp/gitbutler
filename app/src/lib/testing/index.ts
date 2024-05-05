import { baseBranch, project, remoteBranchData, user, virtualBranches } from "./fixtures"
import { mockIPC } from "@tauri-apps/api/mocks";
import { mockWindows } from '@tauri-apps/api/mocks';

export function mockTauri() {
    mockIPC((cmd, args) => {
        console.log("MOCKIPC.CMD", cmd, args)

        // Open Project Dialog
        if (cmd === "tauri" && args.__tauriModule === "Dialog" && args.message?.cmd === "openDialog") {
            return "/Users/user/project"
        }

        // List Projects
        if (cmd === "list_projects") {
            return [project]
        }

        // List Project
        if (cmd === "get_project" && args.id === "ac44a3bb-8bbb-4af9-b8c9-7950dd9ec295") {
            return project
        }

        // Get HEAD
        if (cmd === "git_head") {
            return "refs/heads/gitbutler/integration"
        }

        if (cmd === "menu_item_set_enabled") {
            return null
        }

        if (cmd === "fetch_from_target") {
            return true
        }

        if (cmd === "get_base_branch_data") {
            return baseBranch
        }

        if (cmd === "list_virtual_branches") {
            return virtualBranches
        }

        if (cmd === "list_remote_branches") {
            return []
        }

        if (cmd === "get_remote_branch_data") {
            return remoteBranchData
        }

        if (cmd === "get_remote_branchs") {
            return ["refs/heads/abc123"]
        }

        if (cmd === "get_user") {
            return user
        }
    });
    mockWindows('main');

}
